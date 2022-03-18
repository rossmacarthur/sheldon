//! Top level application implementation.

use std::fs;
use std::io;
use std::path::Path;

use anyhow::{bail, Context as ResultExt, Error, Result};

use crate::cli::{Command, Opt};
use crate::config;
use crate::config::{EditConfig, EditPlugin, Shell};
use crate::context::Context;
use crate::editor;
use crate::lock;
use crate::lock::LockedConfig;
use crate::util::{underlying_io_error_kind, PathExt};

/// Generic function to initialize the config file.
fn init_config(ctx: &Context, shell: Option<Shell>, path: &Path, err: Error) -> Result<EditConfig> {
    if underlying_io_error_kind(&err) == Some(io::ErrorKind::NotFound) {
        if !casual::confirm(format!(
            "Initialize new config file `{}`?",
            &ctx.replace_home(path).display()
        )) {
            bail!("aborted initialization!");
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(s!(
                "failed to create directory `{}`",
                &ctx.replace_home(parent).display()
            ))?;
        }
        Ok(EditConfig::default(shell))
    } else {
        Err(err)
    }
}

/// Executes the `init` subcommand.
///
/// Initialize a new config file.
fn init(ctx: &Context, shell: Option<Shell>) -> Result<()> {
    let path = ctx.config_file();
    match path
        .metadata()
        .with_context(s!("failed to check `{}`", path.display()))
    {
        Ok(_) => {
            header!(ctx, "Already initialized", path);
        }
        Err(err) => {
            init_config(ctx, shell, path, err)?.to_path(path)?;
            header!(ctx, "Initialized", path);
        }
    }
    Ok(())
}

/// Executes the `add` subcommand.
///
/// Add a new plugin to the config file.
fn add(ctx: &Context, name: String, plugin: &EditPlugin) -> Result<()> {
    let path = ctx.config_file();
    let mut config = match EditConfig::from_path(path) {
        Ok(config) => {
            header!(ctx, "Loaded", path);
            config
        }
        Err(err) => init_config(ctx, None, path, err)?,
    };
    config.add(&name, plugin)?;
    status!(ctx, "Added", &name);
    config.to_path(ctx.config_file())?;
    header!(ctx, "Updated", path);
    Ok(())
}

/// Executes the `edit` subcommand.
///
/// Open up the config file in the default editor.
fn edit(ctx: &Context) -> Result<()> {
    let path = ctx.config_file();
    let original_contents = match fs::read_to_string(path)
        .with_context(s!("failed to read from `{}`", path.display()))
    {
        Ok(contents) => {
            EditConfig::from_str(&contents)?;
            header!(ctx, "Loaded", path);
            contents
        }
        Err(err) => {
            let config = init_config(ctx, None, path, err)?;
            config.to_path(path)?;
            header!(ctx, "Initialized", path);
            config.to_string()
        }
    };
    let handle = editor::Editor::default()?.edit(path, &original_contents)?;
    status!(ctx, "Opened", &"config in temporary file for editing");
    let config = handle.wait_and_update(&original_contents)?;
    config.to_path(&path)?;
    header!(ctx, "Updated", path);
    Ok(())
}

/// Executes the `remove` subcommand.
///
/// Remove a plugin from the config file.
fn remove(ctx: &Context, name: String) -> Result<()> {
    let path = ctx.config_file();
    let mut config = EditConfig::from_path(path)?;
    header!(ctx, "Loaded", path);
    config.remove(&name);
    status!(ctx, "Removed", &name);
    config.to_path(ctx.config_file())?;
    header!(ctx, "Updated", path);
    Ok(())
}

/// Reads the config from the config file path, locks it, and returns the
/// locked config.
fn locked(ctx: &Context, warnings: &mut Vec<Error>) -> Result<LockedConfig> {
    let path = ctx.config_file();
    let config = config::from_path(path, warnings).context("failed to load config file")?;
    header!(ctx, "Loaded", path);
    lock::config(ctx, config)
}

/// Execute the `lock` subcommand.
///
/// Install the plugins sources and generate the lock file.
fn lock(ctx: &Context, warnings: &mut Vec<Error>) -> Result<()> {
    let mut locked = locked(ctx, warnings)?;

    if let Some(last) = locked.errors.pop() {
        for err in locked.errors {
            error!(ctx, &err);
        }
        Err(last)
    } else {
        locked.clean(ctx, warnings);
        let path = ctx.lock_file();
        locked.to_path(path).context("failed to write lock file")?;
        header!(ctx, "Locked", path);
        Ok(())
    }
}

/// Execute the `source` subcommand.
///
/// Generate and print out the shell script.
fn source(ctx: &Context, warnings: &mut Vec<Error>, relock: bool) -> Result<()> {
    let config_path = ctx.config_file();
    let lock_path = ctx.lock_file();

    let mut to_path = true;

    let locked_config = if relock || config_path.newer_than(lock_path) {
        locked(ctx, warnings)?
    } else {
        match lock::from_path(lock_path) {
            Ok(locked_config) => {
                if locked_config.verify(ctx) {
                    to_path = false;
                    header_v!(ctx, "Unlocked", lock_path);
                    locked_config
                } else {
                    locked(ctx, warnings)?
                }
            }
            Err(_) => locked(ctx, warnings)?,
        }
    };

    let script = locked_config
        .script(ctx)
        .context("failed to render source")?;

    if to_path && locked_config.errors.is_empty() {
        locked_config.clean(ctx, warnings);
        locked_config
            .to_path(lock_path)
            .context("failed to write lock file")?;
        header!(ctx, "Locked", lock_path);
    } else {
        for err in &locked_config.errors {
            error!(ctx, err);
        }
    }

    print!("{}", script);
    Ok(())
}

fn acquire_mutex(ctx: &Context, path: &Path) -> Result<fmutex::Guard> {
    match fmutex::try_lock(path).with_context(s!("failed to open `{}`", path.display()))? {
        Some(g) => Ok(g),
        None => {
            warning!(
                ctx,
                "Blocking",
                &format!(
                    "waiting for file lock on {}",
                    ctx.replace_home(path).display()
                )
            );
            fmutex::lock(path).with_context(s!("failed to acquire file lock `{}`", path.display()))
        }
    }
}

/// The main entry point to execute the application.
pub fn run() -> Result<()> {
    let Opt { ctx, command } = Opt::from_args();

    let _guard = { acquire_mutex(&ctx, ctx.config_dir())? };

    let mut warnings = Vec::new();
    let result = match command {
        Command::Init { shell } => init(&ctx, shell),
        Command::Add { name, plugin } => add(&ctx, name, &plugin),
        Command::Edit => edit(&ctx),
        Command::Remove { name } => remove(&ctx, name),
        Command::Lock => lock(&ctx, &mut warnings),
        Command::Source { relock } => source(&ctx, &mut warnings, relock),
    };
    for err in &warnings {
        error_w!(ctx, err);
    }
    result.map_err(|err| {
        error!(ctx, &err);
        err
    })
}
