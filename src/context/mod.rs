//! Contextual information.

mod message;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use anyhow::Error;
use serde::{Deserialize, Serialize};
pub use yansi::Color;
use yansi::Paint;

use crate::context::message::ToMessage;
use crate::lock::LockMode;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Context {
    pub version: String,
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub profile: Option<String>,

    #[serde(skip)]
    pub lock_file: PathBuf,
    #[serde(skip)]
    pub clone_dir: PathBuf,
    #[serde(skip)]
    pub download_dir: PathBuf,
    #[serde(skip)]
    pub output: Output,
    #[serde(skip)]
    pub lock_mode: Option<LockMode>,
}

/// The output style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Output {
    /// The requested verbosity of output.
    pub verbosity: Verbosity,
    /// Whether to not use ANSI color codes.
    pub no_color: bool,
}

/// The requested verbosity of output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::Normal
    }
}

impl Context {
    /// The location of the configuration directory.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// The location of the data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The location of the config file.
    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    /// The location of the lock file.
    pub fn lock_file(&self) -> &Path {
        &self.lock_file
    }

    /// The directory to clone git sources to.
    pub fn clone_dir(&self) -> &Path {
        &self.clone_dir
    }

    /// The directory to download remote plugins sources to.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// The profile used for conditional plugins.
    pub fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// Expands the tilde in the given path to the configured user's home
    /// directory.
    pub fn expand_tilde(&self, path: PathBuf) -> PathBuf {
        if let Ok(p) = path.strip_prefix("~") {
            self.home.join(p)
        } else {
            path
        }
    }

    /// Replaces the home directory in the given path with a tilde.
    pub fn replace_home<P>(&self, path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        if let Ok(p) = path.strip_prefix(&self.home) {
            Path::new("~").join(p)
        } else {
            path.to_path_buf()
        }
    }

    pub fn lock_mode(&self) -> LockMode {
        self.lock_mode.unwrap_or(LockMode::Normal)
    }

    pub fn verbosity(&self) -> Verbosity {
        self.output.verbosity
    }

    pub fn log_header(&self, prefix: &str, msg: impl ToMessage) {
        let msg = msg.to_message(self);
        if self.output.no_color {
            eprintln!("[{}] {}", prefix.to_uppercase(), msg);
        } else {
            eprintln!("{} {}", Paint::magenta(prefix).bold(), msg);
        }
    }

    pub fn log_status(&self, color: Color, prefix: &str, msg: impl ToMessage) {
        let msg = msg.to_message(self);
        if self.output.no_color {
            eprintln!("{: >12} {}", format!("[{}]", prefix.to_uppercase()), msg);
        } else {
            eprintln!(
                "{} {}",
                Paint::new(format!("{: >10}", prefix)).fg(color).bold(),
                msg
            );
        }
    }

    pub fn log_error(&self, err: &Error) {
        log_error(self.output.no_color, err)
    }

    pub fn log_error_as_warning(&self, err: &Error) {
        log_error_as_warning(self.output.no_color, err)
    }
}

pub fn log_error(no_color: bool, err: &Error) {
    let pretty = prettyify_error(err);
    if no_color {
        eprintln!("\nERROR: {}", pretty);
    } else {
        eprintln!("\n{} {}", Paint::red("error:").bold(), pretty);
    }
}

pub fn log_error_as_warning(no_color: bool, err: &Error) {
    let pretty = prettyify_error(err);
    if no_color {
        eprintln!("\nWARNING: {}", pretty);
    } else {
        eprintln!("\n{} {}", Paint::yellow("warning:").bold(), pretty);
    }
}

fn prettyify_error(err: &Error) -> String {
    err.chain()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("\n  due to: ")
}
