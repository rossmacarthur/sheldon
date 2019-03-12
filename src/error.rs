//! Error handling for this crate.

use std::{error::Error as _Error, fmt, path::Path, result};

/// A custom result type to use in this crate.
pub type Result<T> = result::Result<T, Error>;

/// The kind of error that occurred.
#[derive(Copy, Clone, Debug)]
pub enum ErrorKind {
    /// Occurs when deserializing fails.
    Deserialize,
    /// Occurs when serializing fails.
    Serialize,
    /// Occurs when there is an invalid config setting.
    Config,
    /// Occurs when downloading fails.
    Download,
    /// Occurs when a template fails to compile.
    Template,
    /// Occurs when a template fails to render.
    Render,
    /// Hints that destructuring should not be exhaustive.
    // Until https://github.com/rust-lang/rust/issues/44109 is stabilized.
    #[doc(hidden)]
    __Nonexhaustive,
}

/// An error struct that holds the error kind, a context message, and optionally
/// a source.
#[derive(Debug)]
pub struct Error {
    /// The type of error.
    kind: ErrorKind,
    /// A description of what happened.
    message: String,
    /// The underlying cause of this error.
    source: Option<Box<dyn _Error>>,
}

impl _Error for Error {
    /// The lower-level source of this error, if any.
    fn source(&self) -> Option<&(dyn _Error + 'static)> {
        self.source.as_ref().map(|e| &**e as &_Error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let due_to = if let Some(e) = self.source() {
            format!("\ndue to: {}", e)
        } else {
            String::new()
        };
        write!(f, "{}{}", self.message, due_to)
    }
}

impl Error {
    /// Returns the kind of error that occurred.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the error message.
    pub fn message(&self) -> &String {
        &self.message
    }

    pub(crate) fn deserialize<E: _Error + 'static>(e: E, path: &Path) -> Self {
        Error {
            kind: ErrorKind::Deserialize,
            message: format!("failed to deserialize from `{}`", path.to_string_lossy()),
            source: Some(Box::new(e)),
        }
    }

    pub(crate) fn serialize<E: _Error + 'static>(e: E, path: &Path) -> Self {
        Error {
            kind: ErrorKind::Serialize,
            message: format!("failed to serialize to `{}`", path.to_string_lossy()),
            source: Some(Box::new(e)),
        }
    }

    pub(crate) fn config_git(url: &str) -> Self {
        Error {
            kind: ErrorKind::Config,
            message: format!("failed to parse `{}` as a Git URL", url),
            source: None,
        }
    }

    pub(crate) fn config_github(repository: &str) -> Self {
        Error {
            kind: ErrorKind::Config,
            message: format!("failed to parse `{}` as a GitHub repository", repository),
            source: None,
        }
    }

    pub(crate) fn download<E: _Error + 'static>(e: E, url: &str) -> Self {
        Error {
            kind: ErrorKind::Download,
            message: format!("failed to git clone {}", url),
            source: Some(Box::new(e)),
        }
    }

    pub(crate) fn template<E: _Error + 'static>(e: E, name: &str) -> Self {
        Error {
            kind: ErrorKind::Template,
            message: format!("failed to compile template `{}`", name),
            source: Some(Box::new(e)),
        }
    }

    pub(crate) fn render<E: _Error + 'static>(e: E, name: &str) -> Self {
        Error {
            kind: ErrorKind::Render,
            message: format!("failed to render template `{}`", name),
            source: Some(Box::new(e)),
        }
    }
}
