#[macro_use]
mod macros;
mod cli;
mod config;
mod context;
mod editor;
mod lock;
mod util;

use std::fs;
use std::io;
use std::panic;
use std::path::Path;
use std::process;

use anyhow::{bail, Context as ResultExt, Error, Result};

use crate::cli::{Command, Opt};
use crate::config::{EditConfig, EditPlugin, Shell};
use crate::context::Context;
use crate::lock::LockedConfig;
use crate::util::underlying_io_error_kind;

fn main() {
    let res = panic::catch_unwind(|| {
        let Opt { ctx, command } = cli::from_args();
        if let Err(err) = run_command(&ctx, command) {
            ctx.log_error(&err);
            process::exit(2);
        }
    });
    if res.is_err() {
        eprintln!(
            "\nThis is probably a bug, please file an issue at \
             https://github.com/rossmacarthur/sheldon/issues."
        );
        process::exit(127);
    }
}

/// The main entry point to execute the application.
pub fn run_command(ctx: &Context, command: Command) -> Result<()> {
    // We always try to acquire the mutex but it is only strictly necessary for
    // the lock and source commands.
    let _guard = match acquire_mutex(ctx, ctx.config_dir()) {
        Ok(g) => Some(g),
        Err(_) if !matches!(command, Command::Lock | Command::Source) => None,
        Err(err) => {
            return Err(err).context("failed to acquire lock on config directory");
        }
    };
    let mut warnings = Vec::new();
    let result = match command {
        Command::Init { shell } => init(ctx, shell),
        Command::Add { name, plugin } => add(ctx, name, &plugin),
        Command::Edit => edit(ctx),
        Command::Remove { name } => remove(ctx, name),
        Command::Lock => lock(ctx, &mut warnings),
        Command::Source => source(ctx, &mut warnings),
    };
    for err in &warnings {
        ctx.log_error_as_warning(err);
    }
    result
}

fn acquire_mutex(ctx: &Context, path: &Path) -> Result<fmutex::Guard> {
    match fmutex::try_lock(path).with_context(|| format!("failed to open `{}`", path.display()))? {
        Some(g) => Ok(g),
        None => {
            ctx.log_warning(
                "Blocking",
                &format!(
                    "waiting for file lock on {}",
                    ctx.replace_home(path).display()
                ),
            );
            fmutex::lock(path)
                .with_context(|| format!("failed to acquire file lock `{}`", path.display()))
        }
    }
}

/// Executes the `init` subcommand.
///
/// Initialize a new config file.
fn init(ctx: &Context, shell: Option<Shell>) -> Result<()> {
    let path = ctx.config_file();
    match path
        .metadata()
        .with_context(|| format!("failed to check `{}`", path.display()))
    {
        Ok(_) => {
            ctx.log_header("Unchanged", path);
        }
        Err(err) => {
            init_config(ctx, shell, path, err)?.to_path(path)?;
            ctx.log_header("Initialized", path);
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
            ctx.log_header("Loaded", path);
            config
        }
        Err(err) => init_config(ctx, None, path, err)?,
    };
    config.add(&name, plugin)?;
    ctx.log_status("Added", &name);
    config.to_path(ctx.config_file())?;
    ctx.log_header("Updated", path);
    Ok(())
}

/// Executes the `edit` subcommand.
///
/// Open up the config file in the default editor.
fn edit(ctx: &Context) -> Result<()> {
    let path = ctx.config_file();
    let original_contents = match fs::read_to_string(path)
        .with_context(|| format!("failed to read from `{}`", path.display()))
    {
        Ok(contents) => {
            EditConfig::from_str(&contents)?;
            ctx.log_header("Loaded", path);
            contents
        }
        Err(err) => {
            let config = init_config(ctx, None, path, err)?;
            config.to_path(path)?;
            ctx.log_header("Initialized", path);
            config.to_string()
        }
    };
    let handle = editor::Editor::default()?.edit(ctx, path, &original_contents)?;
    ctx.log_status("Opened", &"config in temporary file for editing");
    let config = handle.wait_and_update(&original_contents)?;
    config.to_path(path)?;
    ctx.log_header("Updated", path);
    Ok(())
}

/// Executes the `remove` subcommand.
///
/// Remove a plugin from the config file.
fn remove(ctx: &Context, name: String) -> Result<()> {
    let path = ctx.config_file();
    let mut config = EditConfig::from_path(path)?;
    ctx.log_header("Loaded", path);
    config.remove(&name);
    ctx.log_status("Removed", &name);
    config.to_path(ctx.config_file())?;
    ctx.log_header("Updated", path);
    Ok(())
}

/// Generic function to initialize the config file.
fn init_config(ctx: &Context, shell: Option<Shell>, path: &Path, err: Error) -> Result<EditConfig> {
    if underlying_io_error_kind(&err) == Some(io::ErrorKind::NotFound) {
        if ctx.interactive
            && !casual::confirm(format!(
                "Initialize new config file `{}`?",
                &ctx.replace_home(path).display()
            ))
        {
            bail!("aborted initialization!");
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create directory `{}`",
                    &ctx.replace_home(parent).display()
                )
            })?;
        }
        Ok(EditConfig::default(shell))
    } else {
        Err(err)
    }
}

/// Execute the `lock` subcommand.
///
/// Install the plugins sources and generate the lock file.
fn lock(ctx: &Context, warnings: &mut Vec<Error>) -> Result<()> {
    let mut locked = locked(ctx, warnings)?;

    if let Some(last) = locked.errors.pop() {
        for err in locked.errors {
            ctx.log_error(&err);
        }
        Err(last)
    } else {
        let path = ctx.lock_file();
        locked.to_path(path).context("failed to write lock file")?;
        ctx.log_header("Locked", path);
        Ok(())
    }
}

/// Execute the `source` subcommand.
///
/// Generate and print out the shell script.
fn source(ctx: &Context, warnings: &mut Vec<Error>) -> Result<()> {
    let config_path = ctx.config_file();
    let lock_path = ctx.lock_file();

    let mut to_path = true;

    let locked_config = if ctx.lock_mode.is_some() || newer_than(config_path, lock_path) {
        locked(ctx, warnings)?
    } else {
        match lock::from_path(lock_path) {
            Ok(locked_config) => {
                if locked_config.verify(ctx) {
                    to_path = false;
                    ctx.log_verbose_header("Unlocked", lock_path);
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
        locked_config
            .to_path(lock_path)
            .context("failed to write lock file")?;
        ctx.log_header("Locked", lock_path);
    } else {
        for err in &locked_config.errors {
            ctx.log_error(err);
        }
    }

    print!("{script}");
    Ok(())
}

/// Returns `true` if the left path is newer than the right.
fn newer_than(left: &Path, right: &Path) -> bool {
    let modified = |p| fs::metadata(p).and_then(|m| m.modified()).ok();
    match (modified(left), modified(right)) {
        (Some(ltime), Some(rtime)) => ltime > rtime,
        _ => false,
    }
}

/// Reads the config from the config file path, locks it, and returns the
/// locked config.
fn locked(ctx: &Context, warnings: &mut Vec<Error>) -> Result<LockedConfig> {
    let path = ctx.config_file();
    let config = config::from_path(path, warnings).context("failed to load config file")?;
    ctx.log_header("Loaded", path);
    config::clean(ctx, warnings, &config)?;
    lock::config(ctx, config)
}
