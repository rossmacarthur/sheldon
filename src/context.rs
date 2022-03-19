//! Contextual information.

use std::fmt;
use std::path::{Path, PathBuf};

pub use ansi_term::Color;
use anyhow::Error;
use serde::{Deserialize, Serialize};

use crate::lock::LockMode;
use crate::util::PathExt;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Context {
    pub version: String,
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub lock_file: PathBuf,
    pub clone_dir: PathBuf,
    pub download_dir: PathBuf,
    #[serde(skip)]
    pub output: Output,
    #[serde(skip)]
    pub lock_mode: Option<LockMode>,
}

/// The output style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Output {
    /// The requested verbosity of output.
    pub verbosity: Verbosity,
    /// Whether to not use ANSI color codes.
    pub no_color: bool,
}

/// The requested verbosity of output.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

/// A message that can be logged.
pub enum Message<'a> {
    /// A reference to something that can be displayed.
    Borrowed(&'a dyn fmt::Display),
    /// An owned string.
    Owned(String),
}

/// A trait for converting a reference to something into a `Message`.
pub trait ToMessage {
    fn to_message(&self, ctx: &Context) -> Message<'_>;
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::Normal
    }
}

impl Context {
    /// The location of the home directory.
    pub fn home(&self) -> &Path {
        &self.home
    }

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

    /// Expands the tilde in the given path to the configured user's home
    /// directory.
    pub fn expand_tilde(&self, path: PathBuf) -> PathBuf {
        path.expand_tilde(self.home())
    }

    /// Replaces the home directory in the given path with a tilde.
    pub fn replace_home<P>(&self, path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        path.as_ref().replace_home(self.home())
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
            eprintln!("{} {}", Color::Purple.bold().paint(prefix), msg);
        }
    }

    pub fn log_status(&self, color: Color, prefix: &str, msg: impl ToMessage) {
        let msg = msg.to_message(self);
        if self.output.no_color {
            eprintln!("{: >12} {}", format!("[{}]", prefix.to_uppercase()), msg);
        } else {
            eprintln!("{} {}", color.bold().paint(format!("{: >10}", prefix)), msg);
        }
    }

    pub fn log_error(&self, color: Color, prefix: &str, err: &Error) {
        log_error(self.output.no_color, color, prefix, err);
    }
}

impl<'a> fmt::Display for Message<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self {
            Message::Borrowed(b) => fmt::Display::fmt(b, f),
            Message::Owned(o) => fmt::Display::fmt(o, f),
        }
    }
}

impl<T> ToMessage for &T
where
    T: fmt::Display,
{
    /// Anything that implements `Display` can be easily converted into a
    /// `Message` without copying any data.
    fn to_message(&self, _: &Context) -> Message<'_> {
        Message::Borrowed(self)
    }
}

impl ToMessage for &Path {
    /// A reference to a path is converted into a `Message` by replacing a home
    /// path with a tilde. This implementation allocates a new `String` with the
    /// resultant data.
    fn to_message(&self, ctx: &Context) -> Message<'_> {
        Message::Owned(ctx.replace_home(self).display().to_string())
    }
}

pub fn log_error(no_color: bool, color: Color, prefix: &str, err: &Error) {
    let pretty = err
        .chain()
        .map(|c| c.to_string().replace("Template error: ", ""))
        .collect::<Vec<_>>()
        .join("\n  due to: ");
    if no_color {
        eprintln!("\n[{}] {}", prefix.to_uppercase(), pretty);
    } else {
        eprintln!(
            "\n{} {}",
            color.bold().paint(format!("{}:", prefix)),
            pretty
        );
    }
}
