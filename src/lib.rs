//! A fast, configurable, shell plugin manager.
//!
//! This crate provides a command line interface and is not intended to be used
//! as a library. You can install the `sheldon` command line tool using
//!
//! ```sh
//! cargo install sheldon
//! ```
//!
//! Read up more at the project homepage [here][homepage].
//!
//! [homepage]: https://github.com/rossmacarthur/sheldon#sheldon

#[macro_use]
mod _macros;

mod cli;
mod config;
mod context;
mod edit;
mod editor;
mod lock;
mod log;
mod util;

use std::{fs, io, path::Path};

use anyhow::{bail, Context as ResultExt, Error, Result};

use crate::{
    cli::{Command, Opt},
    config::Config,
    context::{Context, EditContext, LockContext, SettingsExt},
    edit::Plugin,
    lock::LockedConfig,
    util::{underlying_io_error_kind, Mutex, PathExt},
};

/// The main application.
#[derive(Debug)]
pub struct Sheldon;

impl Sheldon {
    /// Initialize the config file.
    fn init_config(ctx: &EditContext, path: &Path, err: Error) -> Result<edit::Config> {
        if underlying_io_error_kind(&err) == Some(io::ErrorKind::NotFound) {
            if !casual::confirm(&format!(
                "Initialize new config file `{}`?",
                &ctx.replace_home(path).display()
            )) {
                bail!("Aborted initialization!");
            };
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(s!(
                    "failed to create directory `{}`",
                    &ctx.replace_home(parent).display()
                ))?;
            }
            Ok(edit::Config::default())
        } else {
            Err(err)
        }
    }

    /// Adds a new plugin to the config file.
    fn add(ctx: &EditContext, name: String, plugin: Plugin) -> Result<()> {
        let path = ctx.config_file();
        let mut config = match edit::Config::from_path(path) {
            Ok(config) => {
                header!(ctx, "Loaded", path);
                config
            }
            Err(err) => Self::init_config(ctx, path, err)?,
        };
        config.add(&name, plugin)?;
        status!(ctx, "Added", &name);
        config.to_path(ctx.config_file())?;
        header!(ctx, "Updated", path);
        Ok(())
    }

    /// Opens the config file for editing.
    fn edit(ctx: &EditContext) -> Result<()> {
        let path = ctx.config_file();
        let original_contents = match fs::read_to_string(path)
            .with_context(s!("failed to read from `{}`", path.display()))
        {
            Ok(contents) => {
                edit::Config::from_str(&contents)?;
                header!(ctx, "Loaded", path);
                contents
            }
            Err(err) => {
                let config = Self::init_config(ctx, path, err)?;
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

    /// Removes a plugin from the config file.
    fn remove(ctx: &EditContext, name: String) -> Result<()> {
        let path = ctx.config_file();
        let mut config = edit::Config::from_path(path)?;
        header!(ctx, "Loaded", path);
        config.remove(&name);
        status!(ctx, "Removed", &name);
        config.to_path(ctx.config_file())?;
        header!(ctx, "Updated", path);
        Ok(())
    }

    /// Reads the config from the config file path, locks it, and returns the
    /// locked config.
    fn locked(ctx: &LockContext) -> Result<LockedConfig> {
        let path = ctx.config_file();
        let config = Config::from_path(path).context("failed to load config file")?;
        header!(ctx, "Loaded", path);
        config.lock(ctx)
    }

    /// Locks the config and writes it to the lock file.
    fn lock(ctx: &LockContext) -> Result<()> {
        let mut locked = Self::locked(ctx)?;

        if let Some(last) = locked.errors.pop() {
            for err in locked.errors {
                error!(ctx, &err);
            }
            Err(last)
        } else {
            let warnings = locked.clean(ctx);
            let path = ctx.lock_file();
            locked.to_path(path).context("failed to write lock file")?;
            header!(ctx, "Locked", path);
            for warning in warnings {
                error_w!(ctx, &warning);
            }
            Ok(())
        }
    }

    /// Generates the script.
    fn source(ctx: &LockContext, relock: bool) -> Result<()> {
        let config_path = ctx.config_file();
        let lock_path = ctx.lock_file();

        let mut to_path = true;

        let locked = if relock || config_path.newer_than(lock_path) {
            Self::locked(ctx)?
        } else {
            match LockedConfig::from_path(lock_path) {
                Ok(locked) => {
                    if locked.verify(ctx) {
                        to_path = false;
                        header_v!(ctx, "Unlocked", lock_path);
                        locked
                    } else {
                        Self::locked(ctx)?
                    }
                }
                Err(_) => Self::locked(ctx)?,
            }
        };

        let script = locked.source(ctx).context("failed to render source")?;

        if to_path && locked.errors.is_empty() {
            let warnings = locked.clean(ctx);
            locked
                .to_path(lock_path)
                .context("failed to write lock file")?;
            header_v!(ctx, "Locked", lock_path);
            for warning in warnings {
                error_w!(ctx, &warning);
            }
        } else {
            for err in &locked.errors {
                error!(ctx, &err);
            }
        }

        print!("{}", script);
        Ok(())
    }

    /// Execute based on the configured settings.
    pub fn run() -> Result<()> {
        let Opt {
            settings,
            output,
            command,
        } = Opt::from_args();

        let _mutex = {
            let ctx = Context {
                settings: &settings,
                output: &output,
            };
            Mutex::acquire(&ctx, settings.root())
        };

        match command {
            Command::Add { name, plugin } => {
                let ctx = EditContext { settings, output };
                Self::add(&ctx, name, *plugin)
            }
            Command::Edit => {
                let ctx = EditContext { settings, output };
                Self::edit(&ctx)
            }
            Command::Remove { name } => {
                let ctx = EditContext { settings, output };
                Self::remove(&ctx, name)
            }
            Command::Lock { reinstall } => {
                let ctx = LockContext {
                    settings,
                    output,
                    reinstall,
                };
                Self::lock(&ctx)
            }
            Command::Source { relock, reinstall } => {
                let ctx = LockContext {
                    settings,
                    output,
                    reinstall,
                };
                Self::source(&ctx, relock)
            }
        }
        .map_err(|e| {
            error!(&output, &e);
            e
        })
    }
}
