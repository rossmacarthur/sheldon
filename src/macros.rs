//! General purpose macros.

/// Generate a lazy format!.
macro_rules! s {
    ($fmt:expr, $($arg:tt)+) => (|| format!($fmt, $($arg)+))
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
        if $ctx.verbosity() >= $verbosity {
            $ctx.log_header($status, $message);
        }
    };
}

macro_rules! _status {
    ($verbosity:expr, $color:expr, $ctx:expr, $status:expr, $message:expr) => {
        if $ctx.verbosity() >= $verbosity {
            $ctx.log_status($color, $status, $message);
        }
    };
}

/// Log a pretty header.
macro_rules! header {
    ($($arg:tt)*) => { _header!(crate::context::Verbosity::Normal, $($arg)*) };
}
macro_rules! header_v {
    ($($arg:tt)*) => { _header!(crate::context::Verbosity::Verbose, $($arg)*) };
}

/// Log a status.
macro_rules! status {
    ($($arg:tt)*) => { _status!(crate::context::Verbosity::Normal, crate::context::Color::Cyan, $($arg)*) }
}
macro_rules! status_v {
    ($($arg:tt)*) => { _status!(crate::context::Verbosity::Verbose, crate::context::Color::Cyan, $($arg)*) }
}

/// Log a warning.
macro_rules! warning {
    ($($arg:tt)*) => { _status!(crate::context::Verbosity::Normal, crate::context::Color::Yellow, $($arg)*) }
}
macro_rules! warning_v {
    ($($arg:tt)*) => { _status!(crate::context::Verbosity::Verbose, crate::context::Color::Yellow, $($arg)*) }
}

/// Log an error.
macro_rules! error {
    ($ctx:expr, $err:expr) => {
        $ctx.log_error(crate::context::Color::Red, "error", $err);
    };
}

/// Log an error but as a warning.
macro_rules! error_w {
    ($ctx:expr, $err:expr) => {
        $ctx.log_error(crate::context::Color::Yellow, "warning", $err);
    };
}
