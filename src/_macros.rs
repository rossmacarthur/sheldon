//! General purpose macros.

/// Generate a lazy format!.
macro_rules! s {
    ($fmt:expr, $($arg:tt)+) => (|| format!($fmt, $($arg)+))
}

/// Construct a text only `Err(Error)`.
macro_rules! err {
    ($fmt:expr, $($arg:tt)+) => { Err(crate::error::Error::custom(format!($fmt, $($arg)+))) };
    ($s:tt) => { Err(crate::error::Error::custom($s.into())) };
}

/// Return a text only `Err(Error)`.
macro_rules! bail {
    ($($arg:tt)*) => { return err!($($arg)*); }
}

/// Call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

/// Call .into() on each key and value in a hashmap! initialization.
macro_rules! indexmap_into {
    ($($key:expr => $value:expr),*) => (indexmap!{$($key.into() => $value.into()),*})
}

macro_rules! _header {
    ($verbosity:expr, $ctx:expr, $status:expr, $message:expr) => {
        if crate::log::OutputExt::verbosity($ctx) >= $verbosity {
            crate::log::header($ctx, $status, $message);
        }
    };
}

macro_rules! _status {
    ($verbosity:expr, $color:expr, $ctx:expr, $status:expr, $message:expr) => {
        if crate::log::OutputExt::verbosity($ctx) >= $verbosity {
            crate::log::status($ctx, $color, $status, $message);
        }
    };
}

/// Log a pretty header.
macro_rules! header {
    ($($arg:tt)*) => { _header!(crate::log::Verbosity::Normal, $($arg)+) };
}
macro_rules! header_v {
    ($($arg:tt)*) => { _header!(crate::log::Verbosity::Verbose, $($arg)+) };
}

/// Log a status.
macro_rules! status {
    ($($arg:tt)*) => { _status!(crate::log::Verbosity::Normal, crate::log::Color::Cyan, $($arg)+) }
}
macro_rules! status_v {
    ($($arg:tt)*) => { _status!(crate::log::Verbosity::Verbose, crate::log::Color::Cyan, $($arg)+) }
}

/// Log a warning.
macro_rules! warning {
    ($($arg:tt)*) => { _status!(crate::log::Verbosity::Normal, crate::log::Color::Yellow, $($arg)+) }
}
macro_rules! warning_v {
    ($($arg:tt)*) => { _status!(crate::log::Verbosity::Verbose, crate::log::Color::Yellow, $($arg)+) }
}

/// Log an error.
macro_rules! error {
    ($ctx:expr, $error:expr) => {
        crate::log::error($ctx, crate::log::Color::Red, "error", $error)
    };
}

/// Log an error but as a warning.
macro_rules! error_w {
    ($ctx:expr, $error:expr) => {
        crate::log::error($ctx, crate::log::Color::Yellow, "warning", $error)
    };
}
