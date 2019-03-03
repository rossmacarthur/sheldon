//! Error handling for this crate.

use std::{
    error::{self, Error as _},
    fmt, result,
};

/// A custom result type to use in this crate.
pub type Result<T> = result::Result<T, Error>;

/// The kind of error that occurred.
#[derive(Copy, Clone, Debug)]
pub enum ErrorKind {}

/// An error struct that holds the error kind, a context message, and optionally a source.
pub struct Error {
    /// The type of error..
    kind: ErrorKind,
    /// A description of what happened.
    context: Box<dyn fmt::Display>,
    /// The underlying cause of this error.
    source: Option<Box<dyn error::Error>>,
}

impl error::Error for Error {
    /// The lower-level source of this error, if any.
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source.as_ref().map(|e| &**e as &error::Error)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Error {{ kind: {:?}, context: {}, source: {:?} }}",
            self.kind, self.context, self.source
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let due_to = if let Some(e) = self.source() {
            format!("\ndue to: {}", e)
        } else {
            String::new()
        };
        write!(f, "{}{}", self.context, due_to)
    }
}
