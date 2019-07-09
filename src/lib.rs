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
        let verbosity = if matches.is_present("quiet") {
            Verbosity::Quiet
        } else if matches.is_present("verbose") {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        };

        let command = match subcommand {
            "lock" => Command::Lock,
            "source" => Command::Source,
            _ => panic!("unrecognized command `{}`", subcommand),
        };

        Self {
            verbosity,
            no_color: matches.is_present("no-color"),
            home: matches.value_of("home").map(|s| s.into()),
            root: matches.value_of("root").map(|s| s.into()),
            config_file: matches.value_of("config_file").map(|s| s.into()),
            lock_file: matches.value_of("lock_file").map(|s| s.into()),
            command,
            reinstall: submatches.is_present("reinstall"),
            relock: submatches.is_present("reinstall") || submatches.is_present("relock"),
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

        Sheldon {
            ctx: Context {
                version: crate_version!(),
                verbosity: self.verbosity,
                no_color: self.no_color,
                home,
                root,
                config_file,
                lock_file,
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
        let config = Config::from_path(&path).chain(s!("failed to load config file"))?;
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
            locked
                .to_path(&self.ctx.lock_file)
                .chain(s!("failed to write lock file"))?;
            self.ctx.header(
                "Locked",
                &self.ctx.replace_home(&self.ctx.lock_file).display(),
            );
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

        let script = locked
            .source(&self.ctx)
            .chain(s!("failed to render source"))?;

        for err in &locked.errors {
            self.ctx.error(&err);
        }

        if to_path && locked.errors.is_empty() {
            locked
                .to_path(&self.ctx.lock_file)
                .chain(s!("failed to write lock file"))?;
            self.ctx.header(
                "Locked",
                &self.ctx.replace_home(&self.ctx.lock_file).display(),
            );
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
        if let Err(e) = self.run_command() {
            self.ctx.error(&e);
            Err(e)
        } else {
            Ok(())
        }
    }
}
