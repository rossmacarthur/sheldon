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

#![recursion_limit = "128"]

#[macro_use]
mod _macros;

mod cli;
mod config;
mod context;
mod error;
mod lock;
mod log;
mod util;

use crate::{
    cli::{Command, Opt},
    config::Config,
    context::{Context, LockContext, SettingsExt},
    error::{Result, ResultExt},
    lock::LockedConfig,
    util::{Mutex, PathExt},
};

/// The main application.
#[derive(Debug)]
pub struct Sheldon;

impl Sheldon {
    /// Reads the config from the config file path, locks it, and returns the
    /// locked config.
    fn locked(ctx: &LockContext) -> Result<LockedConfig> {
        let path = ctx.config_file();
        let config = Config::from_path(path).chain("failed to load config file")?;
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
            locked.to_path(path).chain("failed to write lock file")?;
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

        let script = locked.source(ctx).chain("failed to render source")?;

        if to_path && locked.errors.is_empty() {
            let warnings = locked.clean(ctx);
            locked
                .to_path(lock_path)
                .chain("failed to write lock file")?;
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
