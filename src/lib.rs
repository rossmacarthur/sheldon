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
mod error;
#[macro_use]
mod util;

mod config;
mod lock;

use std::{
    env,
    path::{Path, PathBuf},
};

use clap::crate_version;

pub use crate::error::{Error, ErrorKind, Result};
use crate::{
    config::Config,
    context::{Context, Verbosity},
    error::ResultExt,
    lock::LockedConfig,
    util::{PathBufExt, PathExt},
};

mod context {
    use std::{fmt, path::PathBuf};

    use ansi_term::Color;

    use crate::util::PathBufExt;

    /// The requested verbosity of output.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Verbosity {
        Quiet,
        Normal,
        Verbose,
    }

    /// Global contextual information for use over the entire program.
    #[derive(Clone, Debug)]
    pub struct Context {
        /// The requested verbosity of output.
        pub verbosity: Verbosity,
        /// Whether to not use ANSI color codes.
        pub no_color: bool,
        /// The current crate version.
        pub version: &'static str,
        /// The location of the home directory.
        pub home: PathBuf,
        /// The location of the root directory.
        pub root: PathBuf,
        /// The location of the config file.
        pub config_file: PathBuf,
        /// The location of the lock file.
        pub lock_file: PathBuf,
        /// Whether to reinstall plugin sources.
        pub reinstall: bool,
        /// Whether to regenerate the plugins lock file.
        pub relock: bool,
    }

    impl Default for Verbosity {
        fn default() -> Self {
            Verbosity::Normal
        }
    }

    impl Context {
        /// Expands the tilde in the given path to the configured user's home
        /// directory.
        pub fn expand_tilde(&self, path: PathBuf) -> PathBuf {
            path.expand_tilde(&self.home)
        }

        /// Replaces the home directory in the given path to a tilde.
        pub fn replace_home<P>(&self, path: P) -> PathBuf
        where
            P: Into<PathBuf>,
        {
            path.into().replace_home(&self.home)
        }

        fn _header(&self, header: &str, message: &dyn fmt::Display) {
            if self.no_color {
                eprintln!("[{}] {}", header.to_uppercase(), message);
            } else {
                eprintln!("{} {}", Color::Purple.bold().paint(header), message);
            }
        }

        fn _status(&self, status: &str, message: &dyn fmt::Display) {
            if self.no_color {
                eprintln!(
                    "{: >12} {}",
                    format!("[{}]", status.to_uppercase()),
                    message
                )
            } else {
                eprintln!(
                    "{} {}",
                    Color::Cyan.bold().paint(format!("{: >10}", status)),
                    message
                );
            }
        }

        pub fn header(&self, header: &str, message: &dyn fmt::Display) {
            if self.verbosity != Verbosity::Quiet {
                self._header(header, message);
            }
        }

        pub fn header_v(&self, header: &str, message: &dyn fmt::Display) {
            if self.verbosity == Verbosity::Verbose {
                self._header(header, message);
            }
        }

        pub fn status(&self, status: &str, message: &dyn fmt::Display) {
            if self.verbosity != Verbosity::Quiet {
                self._status(status, message);
            }
        }

        pub fn status_v(&self, status: &str, message: &dyn fmt::Display) {
            if self.verbosity == Verbosity::Verbose {
                self._status(status, message);
            }
        }
    }
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
    quiet: bool,
    verbose: bool,
    no_color: bool,
    home: Option<PathBuf>,
    root: Option<PathBuf>,
    config_file: Option<PathBuf>,
    lock_file: Option<PathBuf>,
    reinstall: bool,
    relock: bool,
}

/// The main application.
///
/// This struct is can be created in two ways. Either using the `Default`
/// implementation or with the [`Builder`].
///
/// # Examples
///
/// Using the default settings
///
/// ```rust,ignore
/// let app = sheldon::Sheldon::default();
///
/// println!("{}", app.source()?);
/// ```
///
/// Or with the [`Builder`].
///
/// ```rust,ignore
/// let app = sheldon::Builder::default()
///     .root("~/.config/sheldon")
///     .config_file("~/.plugins.toml")
///     .lock_file("~/.config/sheldon/plugins.lock")
///     .reinstall(true)
///     .verbose(true)
///     .build();
///
/// println!("{}", app.source()?);
/// ```
///
/// [`Builder`]: struct.Builder.html
#[derive(Debug)]
pub struct Sheldon {
    ctx: Context,
}

impl Builder {
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

    /// Whether to suppress output. This defaults to `false`.
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Whether to enable verbose output. This defaults to `false`.
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Whether to output ANSI color codes. This defaults to `false`.
    pub fn no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    /// Create a new `Builder` using parsed command line arguments.
    #[doc(hidden)]
    pub fn from_arg_matches(matches: &clap::ArgMatches, submatches: &clap::ArgMatches) -> Self {
        Self {
            quiet: matches.is_present("quiet"),
            verbose: matches.is_present("verbose"),
            no_color: matches.is_present("no-color"),
            home: matches.value_of("home").map(|s| s.into()),
            root: matches.value_of("root").map(|s| s.into()),
            config_file: matches.value_of("config_file").map(|s| s.into()),
            lock_file: matches.value_of("lock_file").map(|s| s.into()),
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

        let verbosity = {
            use Verbosity::*;
            if self.quiet {
                Quiet
            } else if self.verbose {
                Verbose
            } else {
                Normal
            }
        };

        Sheldon {
            ctx: Context {
                verbosity,
                no_color: self.no_color,
                version: crate_version!(),
                home,
                root,
                config_file,
                lock_file,
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
        let locked = config.lock(&self.ctx).chain(s!("failed to lock config"))?;
        self.ctx.header(
            "Locked",
            &self.ctx.replace_home(&self.ctx.lock_file).display(),
        );
        Ok(locked)
    }

    /// Locks the config and writes it to the lock file.
    pub fn lock(&self) -> Result<()> {
        Ok(self
            .locked()?
            .to_path(&self.ctx.lock_file)
            .chain(s!("failed to write lock file"))?)
    }

    /// Generates the script.
    pub fn source(&self) -> Result<String> {
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

        if to_path {
            locked
                .to_path(&self.ctx.lock_file)
                .chain(s!("failed to write lock file"))?;
        }

        Ok(script)
    }
}
