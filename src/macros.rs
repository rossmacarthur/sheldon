/// Call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

/// Call .into() on each key and value in a hashmap! initialization.
macro_rules! indexmap_into {
    ($($key:expr => $value:expr),*) => (indexmap!{$($key.into() => $value.into()),*})
}
