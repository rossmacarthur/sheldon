//! Error handling.

use std::{error::Error as Error_, fmt, io, result};

use quick_error::quick_error;

/// A custom result type to use in this crate.
pub type Result<T> = result::Result<T, Error>;

/// An error that can occur in this crate.
#[derive(Debug)]
pub struct Error {
    /// The type of error.
    kind: ErrorKind,
    /// A chain of descriptions of what happened.
    messages: Vec<String>,
}

/// A trait to allow easy conversion of other `Result`s into our [`Result`] with
/// a contextual message.
///
/// [`Result`]: type.Result.html
pub trait ResultExt<T, E> {
    fn chain<C, D>(self, c: C) -> Result<T>
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
        Git(err: git2::Error) {
            from()
            source(err)
        }
        /// Occurs when a download fails.
        Download(err: reqwest::Error) {
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
    /// Chain a message to the `Result`.
    fn chain<C, D>(self, c: C) -> Result<T>
    where
        C: FnOnce() -> D,
        D: fmt::Display,
    {
        self.map_err(|e| Error {
            kind: e.into(),
            messages: vec![format!("{}", c())],
        })
    }
}

impl<T> ResultExt<T, Error> for Result<T> {
    /// Chain a message to the `Result`.
    fn chain<C, D>(self, c: C) -> Self
    where
        C: FnOnce() -> D,
        D: fmt::Display,
    {
        self.map_err(|mut e| {
            e.messages.push(format!("{}", c()));
            e
        })
    }
}

impl<T> ResultExt<T, Error> for Option<T> {
    /// Chain a message to the `Option`.
    fn chain<C, D>(self, c: C) -> Result<T>
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

        for message in self.messages.iter().rev() {
            display.push_str(message);
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
    pub(crate) fn custom(message: String) -> Self {
        Self {
            kind: ErrorKind::Custom,
            messages: vec![message],
        }
    }

    /// A pretty representation of this `Error`.
    pub(crate) fn pretty(&self) -> String {
        format!("{}", self)
            .replace("\n", "\n  due to: ")
            .replace("Template error: ", "")
    }
}
