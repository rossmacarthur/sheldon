/// Generate a lazy format!.
macro_rules! s {
    ($($arg:tt)*) => (|| format!($($arg)*))
}

/// Return a text only `Error`.
macro_rules! bail {
    ($fmt:expr, $($arg:tt)+) => {
        return Err(crate::error::Error::custom(format!($fmt, $($arg)+)));
    };
    ($s:tt) => {
        return Err(crate::error::Error::custom($s.into()));
    };
}

/// Call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

/// Call .into() on each key and value in a hashmap! initialization.
macro_rules! indexmap_into {
    ($($key:expr => $value:expr),*) => (indexmap!{$($key.into() => $value.into()),*})
}
