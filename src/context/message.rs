use std::fmt;
use std::path::Path;

use crate::context::Context;

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

impl fmt::Display for Message<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
