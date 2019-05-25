//! A fast, configurable, shell plugin manager.
//!
//! # Features
//!
//! - Can manage almost anything.
//!   - Any public Git repository.
//!     - Branch/tag/commit support.
//!     - Extra support for GitHub repositories.
//!     - Extra support for Gists.
//!   - Arbitrary remote files, simply specify the URL.
//!   - Local plugins, simply specify the directory path.
//! - Highly configurable install methods using [handlebars] templating.
//! - Super-fast parallel installation.
//! - Configuration file using [TOML] syntax.
//! - Uses a lock file for much faster loading of plugins.
//!
//! # Getting started
//!
//! You can install the `sheldon` command line tool using
//!
//! ```sh
//! cargo install sheldon
//! ```
//!
//! Create a configuration file at `~/.zsh/plugins.toml`.
//!
//! ```toml
//! [plugins.oh-my-zsh]
//! source = 'github'
//! repository = 'robbyrussell/oh-my-zsh'
//! ```
//!
//! Read up more about configuration [here][configuration].
//!
//! You can then use the source command to generate the script
//!
//! ```sh
//! # ~/.zshrc
//! source <(sheldon source)
//! ```
//!
//! [configuration]: https://github.com/rossmacarthur/sheldon/blob/master/docs/Configuration.md
//! [handlebars]: http://handlebarsjs.com
//! [toml]: https://github.com/toml-lang/toml

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
    error::ResultExt,
    lock::LockedConfig,
    settings::Settings,
    util::PathExt,
};

mod settings {
    use std::path::PathBuf;

    /// Global settings for use over the entire program.
    #[derive(Clone, Debug)]
    pub struct Settings {
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

    impl Settings {
        /// Expands the tilde in the given path to the configured user's home
        /// directory.
        pub fn expand_tilde(&self, path: PathBuf) -> PathBuf {
            crate::util::expand_tilde_with(path, &self.home)
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
///     .relock(false)
///     .build();
///
/// println!("{}", app.source()?);
/// ```
///
/// [`Builder`]: struct.Builder.html
#[derive(Debug)]
pub struct Sheldon {
    settings: Settings,
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

    /// Create a new `Builder` using parsed command line arguments.
    #[doc(hidden)]
    pub fn from_arg_matches(matches: &clap::ArgMatches) -> Self {
        Self {
            home: matches.value_of("home").map(|s| s.into()),
            root: matches.value_of("root").map(|s| s.into()),
            config_file: matches.value_of("config_file").map(|s| s.into()),
            lock_file: matches.value_of("lock_file").map(|s| s.into()),
            reinstall: matches.is_present("reinstall"),
            relock: matches.is_present("relock"),
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
            opt.map(|p| util::expand_tilde_with(p, home))
                .unwrap_or_else(|| {
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
            settings: Settings {
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
        Ok(Config::from_path(&self.settings.config_file)
            .ctx(s!("failed to load config file"))?
            .lock(&self.settings)
            .ctx(s!("failed to lock config"))?)
    }

    /// Locks the config and writes it to the lock file.
    pub fn lock(&self) -> Result<()> {
        Ok(self
            .locked()?
            .to_path(&self.settings.lock_file)
            .ctx(s!("failed to write lock file"))?)
    }

    /// Generates the script.
    pub fn source(&self) -> Result<String> {
        let mut to_path = true;

        let locked = if self.settings.relock
            || self
                .settings
                .config_file
                .newer_than(&self.settings.lock_file)
        {
            self.locked()?
        } else {
            match LockedConfig::from_path(&self.settings.lock_file) {
                Ok(locked) => {
                    if self.settings == locked.settings {
                        to_path = false;
                        locked
                    } else {
                        self.locked()?
                    }
                }
                Err(_) => self.locked()?,
            }
        };

        let script = locked.source().ctx(s!("failed to render source"))?;

        if to_path {
            locked
                .to_path(&self.settings.lock_file)
                .ctx(s!("failed to write lock file"))?;
        }

        Ok(script)
    }
}
