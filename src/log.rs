//! Logging helpers.

use std::fmt;
use std::path::Path;

pub use ansi_term::Color;
use anyhow::Error;

use crate::context::{Context, EditContext, LockContext, SettingsExt};

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

/// Provides an interface to output information using a struct that contains an
/// `Output`.
pub trait OutputExt {
    fn output(&self) -> &Output;

    /// Returns the requested verbosity of output.
    #[inline]
    fn verbosity(&self) -> Verbosity {
        self.output().verbosity
    }

    /// Returns whether to not use ANSI color codes.
    #[inline]
    fn no_color(&self) -> bool {
        self.output().no_color
    }
}

/// A message that can be logged.
pub enum Message<'a> {
    /// A reference to something that can be displayed.
    Borrowed(&'a dyn fmt::Display),
    /// An owned string.
    Owned(String),
}

/// A trait for converting a reference to something into a `Message`.
pub trait IntoMessage {
    fn to_message<C>(&self, ctx: &C) -> Message<'_>
    where
        C: SettingsExt;
}

impl OutputExt for Output {
    #[inline]
    fn output(&self) -> &Output {
        self
    }
}

impl OutputExt for Context<'_> {
    #[inline]
    fn output(&self) -> &Output {
        self.output
    }
}

impl OutputExt for EditContext {
    #[inline]
    fn output(&self) -> &Output {
        &self.output
    }
}

impl OutputExt for LockContext {
    #[inline]
    fn output(&self) -> &Output {
        &self.output
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

impl<T> IntoMessage for &T
where
    T: fmt::Display,
{
    /// Anything that implements `Display` can be easily converted into a
    /// `Message` without copying any data.
    fn to_message<C>(&self, _: &C) -> Message<'_>
    where
        C: SettingsExt,
    {
        Message::Borrowed(self)
    }
}

impl IntoMessage for &Path {
    /// A reference to a path is converted into a `Message` by replacing a home
    /// path with a tilde. This implementation allocates a new `String` with the
    /// resultant data.
    fn to_message<C>(&self, ctx: &C) -> Message<'_>
    where
        C: SettingsExt,
    {
        Message::Owned(ctx.replace_home(self).display().to_string())
    }
}

/// Log a `Message` as a header.
pub fn header<C, M>(ctx: &C, status: &str, message: M)
where
    C: SettingsExt + OutputExt,
    M: IntoMessage,
{
    let message = message.to_message(ctx);
    if ctx.no_color() {
        eprintln!("[{}] {}", status.to_uppercase(), message);
    } else {
        eprintln!("{} {}", Color::Purple.bold().paint(status), message);
    }
}

/// Log a `Message` as a status.
pub fn status<C, M>(ctx: &C, color: Color, status: &str, message: M)
where
    C: SettingsExt + OutputExt,
    M: IntoMessage,
{
    let message = message.to_message(ctx);
    if ctx.no_color() {
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

/// Log an `Error`.
pub fn error<C>(ctx: &C, color: Color, status: &str, error: &Error)
where
    C: OutputExt,
{
    let pretty = error
        .chain()
        .map(|c| c.to_string().replace("Template error: ", ""))
        .collect::<Vec<_>>()
        .join("\n  due to: ");
    if ctx.no_color() {
        eprintln!("\n[{}] {}", status.to_uppercase(), pretty);
    } else {
        eprintln!(
            "\n{} {}",
            color.bold().paint(format!("{}:", status)),
            pretty
        );
    }
}
