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
    pub(crate) fn deserialize<E: _Error + 'static>(e: E, path: &Path) -> Self {
        Error {
            kind: ErrorKind::Deserialize,
            message: format!("failed to deserialize from `{}`", path.to_string_lossy()),
            source: Some(Box::new(e)),
        }
    }
}
