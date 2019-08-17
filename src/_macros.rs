/// Generate a lazy format!.
macro_rules! s {
    ($($arg:tt)*) => (|| format!($($arg)*))
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
