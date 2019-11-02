use std::{fmt, path::PathBuf};

use ansi_term::Color;

use crate::{error::Error, util::PathBufExt};

/// The command that we are executing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Lock,
    Source,
}

/// The requested verbosity of output.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

/// Global contextual information for use over the entire program.
#[derive(Debug)]
pub struct Context {
    /// The current crate version.
    pub version: &'static str,
    /// The requested verbosity of output.
    pub verbosity: Verbosity,
    /// Whether to not use ANSI color codes.
    pub no_color: bool,
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
    /// The command that we are executing.
    pub command: Command,
    /// Whether to reinstall plugin sources.
    pub reinstall: bool,
    /// Whether to regenerate the plugins lock file.
    pub relock: bool,
}

impl Default for Command {
    fn default() -> Self {
        Self::Source
    }
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::Normal
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

    fn log_header(&self, header: &str, message: &dyn fmt::Display) {
        if self.no_color {
            eprintln!("[{}] {}", header.to_uppercase(), message);
        } else {
            eprintln!("{} {}", Color::Purple.bold().paint(header), message);
        }
    }

    fn log_status(&self, color: Color, status: &str, message: &dyn fmt::Display) {
        if self.no_color {
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

    pub fn header(&self, header: &str, message: &dyn fmt::Display) {
        if self.verbosity > Verbosity::Quiet {
            self.log_header(header, message);
        }
    }

    pub fn header_v(&self, header: &str, message: &dyn fmt::Display) {
        if self.verbosity > Verbosity::Normal {
            self.log_header(header, message);
        }
    }

    pub fn status(&self, status: &str, message: &dyn fmt::Display) {
        if self.verbosity > Verbosity::Quiet {
            self.log_status(Color::Cyan, status, message);
        }
    }

    pub fn status_v(&self, status: &str, message: &dyn fmt::Display) {
        if self.verbosity > Verbosity::Normal {
            self.log_status(Color::Cyan, status, message);
        }
    }

    pub fn warning(&self, status: &str, message: &dyn fmt::Display) {
        if self.verbosity > Verbosity::Quiet {
            self.log_status(Color::Yellow, status, message);
        }
    }

    pub fn warning_v(&self, status: &str, message: &dyn fmt::Display) {
        if self.verbosity > Verbosity::Normal {
            self.log_status(Color::Yellow, status, message);
        }
    }

    pub fn error_warning(&self, error: &Error) {
        if self.no_color {
            eprintln!("\n[WARNING] {}", error.pretty());
        } else {
            eprintln!(
                "\n{} {}",
                Color::Yellow.bold().paint("warning:"),
                error.pretty()
            );
        }
    }

    pub fn error(&self, error: &Error) {
        if self.no_color {
            eprintln!("\n[ERROR] {}", error.pretty());
        } else {
            eprintln!("\n{} {}", Color::Red.bold().paint("error:"), error.pretty());
        }
    }
}
