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

mod config;
mod context;
mod error;
mod lock;
mod util;

use std::{
    env,
    path::{Path, PathBuf},
};

use clap::crate_version;

use crate::{
    config::Config,
    context::Context,
    error::ResultExt,
    lock::LockedConfig,
    util::{FileMutex, PathBufExt, PathExt},
};
pub use crate::{
    context::{Command, Verbosity},
    error::{Error, ErrorKind, Result},
};

#[doc(hidden)]
pub mod cli {
    // Commands
    pub const LOCK: &str = "lock";
    pub const SOURCE: &str = "source";

    // Flags
    pub const QUIET: &str = "quiet";
    pub const VERBOSE: &str = "verbose";
    pub const NO_COLOR: &str = "no-color";

    // Common options
    pub const HOME: &str = "home";
    pub const ROOT: &str = "root";
    pub const CONFIG_FILE: &str = "config-file";
    pub const LOCK_FILE: &str = "lock-file";
    pub const CLONE_DIR: &str = "clone-dir";
    pub const DOWNLOAD_DIR: &str = "download-dir";

    // Subcommand options
    pub const REINSTALL: &str = "reinstall";
    pub const RELOCK: &str = "relock";
}

/// A builder that is used to construct a [`Sheldon`] with specific settings.
///
/// Settings are set using the "builder pattern". All settings are optional and
/// may be given in any order. When the [`build()`] method is called the
/// `Builder` is consumed, defaults are applied for settings that aren't
/// given, and a [`Sheldon`] object is then generated.
///
/// [`build()`]: struct.Builder.html#method.build
/// [`Sheldon`]: struct.Sheldon.html
#[derive(Debug, Default)]
pub struct Builder {
    verbosity: Verbosity,
    no_color: bool,
    home: Option<PathBuf>,
    root: Option<PathBuf>,
    config_file: Option<PathBuf>,
    lock_file: Option<PathBuf>,
    clone_dir: Option<PathBuf>,
    download_dir: Option<PathBuf>,
    command: Command,
    reinstall: bool,
    relock: bool,
}

/// The main application.
///
/// This struct is can be created in two ways. Either using the `Default`
/// implementation or with the [`Builder`].
///
/// [`Builder`]: struct.Builder.html
#[derive(Debug)]
pub struct Sheldon {
    ctx: Context,
}

impl Builder {
    /// Set the verbosity level. This defaults to `Verbosity::Normal`.
    pub fn verbosity(mut self, verbosity: Verbosity) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Whether to suppress the use of ANSI color codes. This defaults to
    /// `false`.
    pub fn no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    /// Set the current user's home directory.
    ///
    /// If not given, this setting is determined automatically using
    /// [`dirs::home_dir()`]. You should only have to set this setting if your
    /// operating system is unusual.
    ///
    /// This directory will be used to automatically determine defaults for
    /// other settings and to expand tildes in paths given in the config file.
    ///
    /// [`dirs::home_dir()`]: ../dirs/fn.home_dir.html
    pub fn home<P>(mut self, home: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.home = Some(home.into());
        self
    }

    /// Set the root directory.
    ///
    /// If not given, this setting is determined using the following priority:
    /// - The value of the `SHELDON_ROOT` environment variable.
    /// - The `.zsh` directory in the home directory.
    ///
    /// This directory will be used to automatically determine defaults for the
    /// config file and the lock file.
    pub fn root<P>(mut self, root: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.root = Some(root.into());
        self
    }

    /// Set the location of the config file.
    ///
    /// If not given, this setting is determined using the following priority:
    /// - The value of the `SHELDON_CONFIG_FILE` environment variable.
    /// - The `plugins.toml` file in the root directory.
    ///
    /// This filename will be used to automatically determine the default
    /// location of the lock file.
    pub fn config_file<P>(mut self, config_file: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.config_file = Some(config_file.into());
        self
    }

    /// Set the location of the lock file.
    ///
    /// If not given, this setting is determined using the following priority:
    /// - The value of the `SHELDON_LOCK_FILE` environment variable.
    /// - The name of the config file with the extension replaced (or added)
    ///   with `.lock`.
    pub fn lock_file<P>(mut self, lock_file: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.lock_file = Some(lock_file.into());
        self
    }

    /// Set the location of the clone directory.
    ///
    /// If not given, this setting is determined using the following priority:
    /// - The value of the `SHELDON_CLONE_DIR` environment variable.
    /// - The `repositories` directory in the root directory.
    pub fn clone_dir<P>(mut self, clone_dir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.clone_dir = Some(clone_dir.into());
        self
    }

    /// Set the location of the download directory.
    ///
    /// If not given, this setting is determined using the following priority:
    /// - The value of the `SHELDON_DOWNLOAD_DIR` environment variable.
    /// - The `downloads` directory in the root directory.
    pub fn download_dir<P>(mut self, download_dir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.download_dir = Some(download_dir.into());
        self
    }

    /// Set the command to run. This defaults to `Command::Source`.
    pub fn command(mut self, command: Command) -> Self {
        self.command = command;
        self
    }

    /// Whether to reinstall plugin sources. This defaults to `false`.
    pub fn reinstall(mut self, reinstall: bool) -> Self {
        self.reinstall = reinstall;
        self
    }

    /// Whether to relock plugins even if a lock file is found. This defaults to
    /// `false`.
    pub fn relock(mut self, relock: bool) -> Self {
        self.relock = relock;
        self
    }

    /// Create a new `Builder` using parsed command line arguments.
    #[doc(hidden)]
    pub fn from_clap(
        matches: &clap::ArgMatches,
        subcommand: &str,
        submatches: &clap::ArgMatches,
    ) -> Self {
        let verbosity = if matches.is_present(crate::cli::QUIET) {
            Verbosity::Quiet
        } else if matches.is_present(crate::cli::VERBOSE) {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        };

        let command = match subcommand {
            crate::cli::LOCK => Command::Lock,
            crate::cli::SOURCE => Command::Source,
            _ => panic!("unrecognized command `{}`", subcommand),
        };

        Self {
            verbosity,
            no_color: matches.is_present(crate::cli::NO_COLOR),
            home: matches.value_of(crate::cli::HOME).map(|s| s.into()),
            root: matches.value_of(crate::cli::ROOT).map(|s| s.into()),
            config_file: matches.value_of(crate::cli::CONFIG_FILE).map(|s| s.into()),
            lock_file: matches.value_of(crate::cli::LOCK_FILE).map(|s| s.into()),
            clone_dir: matches.value_of(crate::cli::CLONE_DIR).map(|s| s.into()),
            download_dir: matches.value_of(crate::cli::DOWNLOAD_DIR).map(|s| s.into()),
            command,
            reinstall: submatches.is_present(crate::cli::REINSTALL),
            relock: submatches.is_present(crate::cli::REINSTALL)
                || submatches.is_present(crate::cli::RELOCK),
        }
    }

    /// Consume the `Builder`, apply default settings, and create a new
    /// [`Sheldon`].
    ///
    /// [`Sheldon`]: struct.Sheldon.html
    pub fn build(self) -> Sheldon {
        let home = self.home.unwrap_or_else(|| {
            dirs::home_dir().expect("failed to determine the current user's home directory")
        });

        fn process<F>(opt: Option<PathBuf>, home: &Path, var: &str, f: F) -> PathBuf
        where
            F: FnOnce() -> PathBuf,
        {
            opt.map(|p| p.expand_tilde(home)).unwrap_or_else(|| {
                env::var(var)
                    .and_then(|v| Ok(v.into()))
                    .unwrap_or_else(|_| f())
            })
        }

        let root = process(self.root, &home, "SHELDON_ROOT", || home.join(".zsh"));
        let config_file = process(self.config_file, &home, "SHELDON_CONFIG_FILE", || {
            root.join("plugins.toml")
        });
        let lock_file = process(self.lock_file, &home, "SHELDON_LOCK_FILE", || {
            config_file.with_extension("lock")
        });
        let clone_dir = process(self.clone_dir, &home, "SHELDON_CLONE_DIR", || {
            root.join("repositories")
        });
        let download_dir = process(self.download_dir, &home, "SHELDON_DOWNLOAD_DIR", || {
            root.join("downloads")
        });

        Sheldon {
            ctx: Context {
                version: crate_version!(),
                verbosity: self.verbosity,
                no_color: self.no_color,
                home,
                root,
                config_file,
                lock_file,
                clone_dir,
                download_dir,
                command: self.command,
                reinstall: self.reinstall,
                relock: self.relock,
            },
        }
    }
}

impl Default for Sheldon {
    /// Create a new `Sheldon` with the default settings.
    ///
    /// To create a `Sheldon` with modified settings you should use the
    /// [`Builder`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// let app = sheldon::Sheldon::default();
    /// ```
    ///
    /// [`Builder`]: struct.Builder.html
    fn default() -> Self {
        Builder::default().build()
    }
}

impl Sheldon {
    /// Reads the config from the config file path, locks it, and returns the
    /// locked config.
    fn locked(&self) -> Result<LockedConfig> {
        let path = &self.ctx.config_file;
        let config = Config::from_path(&path).chain("failed to load config file")?;
        self.ctx
            .header("Loaded", &self.ctx.replace_home(path).display());
        config.lock(&self.ctx)
    }

    /// Locks the config and writes it to the lock file.
    fn lock(&self) -> Result<()> {
        let mut locked = self.locked()?;

        if let Some(last) = locked.errors.pop() {
            for err in locked.errors {
                self.ctx.error(&err)
            }
            Err(last)
        } else {
            let warnings = locked.clean(&self.ctx);
            locked
                .to_path(&self.ctx.lock_file)
                .chain("failed to write lock file")?;
            self.ctx.header(
                "Locked",
                &self.ctx.replace_home(&self.ctx.lock_file).display(),
            );
            for warning in warnings {
                self.ctx.error_warning(&warning);
            }
            Ok(())
        }
    }

    /// Generates the script.
    fn source(&self) -> Result<()> {
        let mut to_path = true;

        let locked = if self.ctx.relock || self.ctx.config_file.newer_than(&self.ctx.lock_file) {
            self.locked()?
        } else {
            match LockedConfig::from_path(&self.ctx.lock_file) {
                Ok(locked) => {
                    if locked.verify(&self.ctx) {
                        to_path = false;
                        self.ctx.header_v(
                            "Unlocked",
                            &self.ctx.replace_home(&self.ctx.lock_file).display(),
                        );
                        locked
                    } else {
                        self.locked()?
                    }
                }
                Err(_) => self.locked()?,
            }
        };

        let script = locked.source(&self.ctx).chain("failed to render source")?;

        if to_path && locked.errors.is_empty() {
            let warnings = locked.clean(&self.ctx);
            locked
                .to_path(&self.ctx.lock_file)
                .chain("failed to write lock file")?;
            self.ctx.header(
                "Locked",
                &self.ctx.replace_home(&self.ctx.lock_file).display(),
            );
            for warning in warnings {
                self.ctx.error_warning(&warning);
            }
        } else {
            for err in &locked.errors {
                self.ctx.error(&err);
            }
        }

        print!("{}", script);
        Ok(())
    }

    /// Run the configured command.
    fn run_command(&self) -> Result<()> {
        let _mutex = FileMutex::acquire(&self.ctx, &self.ctx.root);
        match self.ctx.command {
            Command::Lock => self.lock(),
            Command::Source => self.source(),
        }
    }

    /// Execute based on the configured settings.
    pub fn run(&self) -> Result<()> {
        self.run_command().map_err(|e| {
            self.ctx.error(&e);
            e
        })
    }
}
