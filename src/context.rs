//! Contextual information.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use ansi_term::Color;
use serde::{Deserialize, Serialize};

use crate::{error::Error, util::PathBufExt};

/// The requested verbosity of output.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

/// The output style.
#[derive(Clone, Copy, Debug)]
pub struct Output {
    /// The requested verbosity of output.
    pub verbosity: Verbosity,
    /// Whether to not use ANSI color codes.
    pub no_color: bool,
}

/// Settings to use over the entire program.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Settings {
    /// The current crate version.
    pub version: String,
    /// The location of the home directory.
    pub home: PathBuf,
    /// The location of the root directory.
    pub root: PathBuf,
    /// The location of the config file.
    pub config_file: PathBuf,
    /// The location of the lock file.
    pub lock_file: PathBuf,
    /// The directory to clone git sources to.
    pub clone_dir: PathBuf,
    /// The directory to download remote plugins sources to.
    pub download_dir: PathBuf,
}

/// Contextual information to use across the entire program.
#[derive(Debug)]
pub struct Context<'a> {
    /// Common data.
    pub settings: &'a Settings,
    /// The output style.
    pub output: &'a Output,
}

/// Contextual information to use while running the main tasks (lock and
/// source).
#[derive(Debug)]
pub struct LockContext {
    /// Common data.
    pub settings: Settings,
    /// The output style.
    pub output: Output,
    /// Whether to reinstall plugin sources.
    pub reinstall: bool,
}

/// Provides an interface to output information using a struct that has an
/// `Output`.
pub trait OutputExt {
    fn output(&self) -> &Output;

    #[inline]
    fn log_header(&self, header: &str, message: &dyn fmt::Display, verbosity: Verbosity) {
        if self.output().verbosity >= verbosity {
            if self.output().no_color {
                eprintln!("[{}] {}", header.to_uppercase(), message);
            } else {
                eprintln!("{} {}", Color::Purple.bold().paint(header), message);
            }
        }
    }

    #[inline]
    fn log_status(
        &self,
        color: Color,
        status: &str,
        message: &dyn fmt::Display,
        verbosity: Verbosity,
    ) {
        if self.output().verbosity >= verbosity {
            if self.output().no_color {
                eprintln!(
                    "{: >12} {}",
                    format!("[{}]", status.to_uppercase()),
                    message
                )
            } else {
                eprintln!(
                    "{} {}",
                    color.bold().paint(format!("{: >10}", status)),
                    message
                );
            }
        }
    }

    #[inline]
    fn header(&self, header: &str, message: &dyn fmt::Display) {
        self.log_header(header, message, Verbosity::Normal);
    }

    #[inline]
    fn header_v(&self, header: &str, message: &dyn fmt::Display) {
        self.log_header(header, message, Verbosity::Verbose);
    }

    #[inline]
    fn status(&self, status: &str, message: &dyn fmt::Display) {
        self.log_status(Color::Cyan, status, message, Verbosity::Normal);
    }

    #[inline]
    fn status_v(&self, status: &str, message: &dyn fmt::Display) {
        self.log_status(Color::Cyan, status, message, Verbosity::Verbose);
    }

    #[inline]
    fn warning(&self, status: &str, message: &dyn fmt::Display) {
        self.log_status(Color::Yellow, status, message, Verbosity::Normal);
    }

    #[inline]
    fn warning_v(&self, status: &str, message: &dyn fmt::Display) {
        self.log_status(Color::Yellow, status, message, Verbosity::Verbose);
    }

    #[inline]
    fn error_warning(&self, error: &Error) {
        if self.output().no_color {
            eprintln!("\n[WARNING] {}", error.pretty());
        } else {
            eprintln!(
                "\n{} {}",
                Color::Yellow.bold().paint("warning:"),
                error.pretty()
            );
        }
    }

    #[inline]
    fn error(&self, error: &Error) {
        if self.output().no_color {
            eprintln!("\n[ERROR] {}", error.pretty());
        } else {
            eprintln!("\n{} {}", Color::Red.bold().paint("error:"), error.pretty());
        }
    }
}

impl OutputExt for Output {
    #[inline]
    fn output(&self) -> &Output {
        self
    }
}

impl OutputExt for &Context<'_> {
    #[inline]
    fn output(&self) -> &Output {
        self.output
    }
}

impl OutputExt for &LockContext {
    #[inline]
    fn output(&self) -> &Output {
        &self.output
    }
}

macro_rules! setting_access {
    ($name:ident) => {
        #[inline]
        fn $name(&self) -> &Path {
            self.settings().$name.as_path()
        }
    };
}

/// Provides an interface to access `Settings` attributes.
pub trait SettingsExt {
    fn settings(&self) -> &Settings;

    setting_access!(home);

    setting_access!(root);

    setting_access!(config_file);

    setting_access!(lock_file);

    setting_access!(clone_dir);

    setting_access!(download_dir);

    /// Expands the tilde in the given path to the configured user's home
    /// directory.
    #[inline]
    fn expand_tilde(&self, path: PathBuf) -> PathBuf {
        path.expand_tilde(self.home())
    }

    /// Replaces the home directory in the given path to a tilde.
    #[inline]
    fn replace_home<P>(&self, path: P) -> PathBuf
    where
        P: Into<PathBuf>,
    {
        path.into().replace_home(self.home())
    }
}

impl SettingsExt for Settings {
    #[inline]
    fn settings(&self) -> &Settings {
        self
    }
}

impl SettingsExt for &Context<'_> {
    #[inline]
    fn settings(&self) -> &Settings {
        self.settings
    }
}

impl SettingsExt for LockContext {
    #[inline]
    fn settings(&self) -> &Settings {
        &self.settings
    }
}
