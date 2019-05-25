//! Error handling.

use std::{error::Error as Error_, fmt, io, result};

use quick_error::quick_error;

/// A simple macro to generate a lazy format!.
macro_rules! s {
    ($($arg:tt)*) => (|| format!($($arg)*))
}

/// A simple macro to return a text only `Error`.
macro_rules! bail {
    ($fmt:expr, $($arg:tt)+) => { return Err(Error::custom(format!($fmt, $($arg)+))); }
}

/// A custom result type to use in this crate.
pub type Result<T> = result::Result<T, Error>;

/// An error that can occur in this crate.
#[derive(Debug)]
pub struct Error {
    /// The type of error.
    kind: ErrorKind,
    /// A chain of contextual descriptions of what happened.
    contexts: Vec<String>,
}

/// A trait to allow easy conversion of other `Result`s into our [`Result`] with
/// a contextual message.
///
/// [`Result`]: type.Result.html
pub trait ResultExt<T, E> {
    fn ctx<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display;
}

quick_error! {
    /// A kind of [`Error`].
    ///
    /// [`Error`]: struct.Error.html
    #[derive(Debug)]
    pub enum ErrorKind {
        /// Occurs when we create an `ErrorKind` from a string.
        Custom {}
        /// Occurs when a URL fails to parse.
        Url(err: url::ParseError) {
            from()
            source(err)
        }
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
        /// Occurs when a download fails.
        Download(err: request::Error) {
            from()
            source(err)
        }
        /// Occurs when a glob pattern fails to parse.
        Glob(err: glob::GlobError) {
            from()
            source(err)
        }
        /// Occurs when a glob pattern fails to parse.
        Pattern(err: glob::PatternError) {
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
    /// Add context to the `Result`.
    fn ctx<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display,
    {
        self.map_err(|e| Error {
            kind: e.into(),
            contexts: vec![format!("{}", c())],
        })
    }
}

impl<T> ResultExt<T, Error> for Result<T> {
    /// Add context to the `Result`.
    fn ctx<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display,
    {
        self.map_err(|mut e| {
            e.contexts.push(format!("{}", c()));
            e
        })
    }
}

impl<T> ResultExt<T, Error> for Option<T> {
    /// Add context to the `Option`.
    fn ctx<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display,
    {
        self.ok_or_else(|| Error::custom(format!("{}", c())))
    }
}

impl Error_ for Error {
    /// The lower-level source of this `Error`, if any.
    fn source(&self) -> Option<&(dyn Error_ + 'static)> {
        self.kind.source()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut display = String::new();

        for context in self.contexts.iter().rev() {
            display.push_str(context);
            display.push('\n');
        }
        if let Some(e) = self.source() {
            display.push_str(&format!("{}", e))
        }

        write!(f, "{}", display.trim_end())?;

        Ok(())
    }
}

impl Error {
    /// Create an `Error` from a custom message.
    pub(crate) fn custom(context: String) -> Self {
        Error {
            kind: ErrorKind::Custom,
            contexts: vec![context],
        }
    }
}
