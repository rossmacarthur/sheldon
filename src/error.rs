//! Error handling for this crate.

use std::{error::Error as _Error, fmt, io, result};

use quick_error::quick_error;

/// A custom result type to use in this crate.
pub type Result<T> = result::Result<T, Error>;

/// A trait to allow easy conversion of other `Result`s into our [`Result`] with
/// a context.
///
/// [`Result`]: type.Result.html
pub(crate) trait ResultExt<T, E> {
    fn context<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display;
}

/// An error that can occur in this crate.
#[derive(Debug)]
pub struct Error {
    /// The type of error.
    kind: ErrorKind,
    /// A description of what happened.
    message: String,
}

quick_error! {
    /// A kind of [`Error`].
    ///
    /// [`Error`]: struct.Error.html
    #[derive(Debug)]
    pub enum ErrorKind {
        /// Occurs when there is an invalid config setting.
        Config {}
        /// Occurs when there is an IO error.
        Io(err: io::Error) {
            from()
            source(err)
        }
        /// Occurs when TOML deserialization fails.
        FromToml(err: toml::de::Error) {
            from()
            source(err)
        }
        /// Occurs when TOML serialization fails.
        ToToml(err: toml::ser::Error) {
            from()
            source(err)
        }
        /// Occurs when there are Git related failures.
        Git(err: git::Error) {
            from()
            source(err)
        }
        /// Occurs when a template fails to compile.
        Template(err: handlebars::TemplateError) {
            from()
            source(err)
        }
        /// Occurs when a template fails to render.
        TemplateRender(err: handlebars::TemplateRenderError) {
            from()
            source(err)
        }
        /// Occurs when a compiled template fails to render.
        Render(err: handlebars::RenderError) {
            from()
            source(err)
        }
        /// Hints that destructuring should not be exhaustive.
        // Until https://github.com/rust-lang/rust/issues/44109 is stabilized.
        #[doc(hidden)]
        __Nonexhaustive {}
    }
}

impl<T, E> ResultExt<T, E> for result::Result<T, E>
where
    E: Into<ErrorKind>,
{
    fn context<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display,
    {
        self.map_err(|e| Error {
            kind: e.into(),
            message: format!("{}", c()),
        })
    }
}

impl _Error for Error {
    /// The lower-level source of this `Error`, if any.
    fn source(&self) -> Option<&(dyn _Error + 'static)> {
        self.kind.source()
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
    /// Returns this `Error`'s message.
    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn source_is_io_not_found(&self) -> bool {
        if let ErrorKind::Io(error) = &self.kind {
            if error.kind() == io::ErrorKind::NotFound {
                return true;
            }
        }
        false
    }

    pub(crate) fn config_git(url: &str) -> Self {
        Error {
            kind: ErrorKind::Config,
            message: format!("failed to parse `{}` as a Git URL", url),
        }
    }

    pub(crate) fn config_github(repository: &str) -> Self {
        Error {
            kind: ErrorKind::Config,
            message: format!("failed to parse `{}` as a GitHub repository", repository),
        }
    }
}
