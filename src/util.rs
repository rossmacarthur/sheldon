//! Utility macros and functions.

use std::{
    fs,
    path::{Path, PathBuf},
    time,
};

/// A macro to call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

/// An extension trait for [`Path`] types.
///
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
pub trait PathExt {
    fn metadata_modified(&self) -> Option<time::SystemTime>;
    fn newer_than(&self, other: &Path) -> bool;
}

impl PathExt for Path {
    /// Returns the modified time of the file if available.
    fn metadata_modified(&self) -> Option<time::SystemTime> {
        fs::metadata(&self).ok().and_then(|m| m.modified().ok())
    }

    /// Returns whether the file at this path is newer than the file at the
    /// given one. If either file does not exist, this method returns `false`.
    fn newer_than(&self, other: &Path) -> bool {
        match (self.metadata_modified(), other.metadata_modified()) {
            (Some(self_time), Some(other_time)) => self_time > other_time,
            _ => false,
        }
    }
}

/// Expands the tilde in the given path to the given home.
pub fn expand_tilde_with(path: PathBuf, home: &Path) -> PathBuf {
    if let Ok(path) = path.strip_prefix("~") {
        home.join(path)
    } else {
        path
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_with_root() {
        assert_eq!(
            expand_tilde_with(PathBuf::from("/"), &Path::new("/test")),
            PathBuf::from("/")
        )
    }

    #[test]
    fn expand_tilde_with_folder_in_root() {
        assert_eq!(
            expand_tilde_with(PathBuf::from("/fol/der"), &Path::new("/test")),
            PathBuf::from("/fol/der")
        )
    }

    #[test]
    fn expand_tilde_with_home() {
        assert_eq!(
            expand_tilde_with(PathBuf::from("~/"), &Path::new("/test")),
            PathBuf::from("/test")
        )
    }

    #[test]
    fn expand_tilde_with_folder_in_home() {
        assert_eq!(
            expand_tilde_with(PathBuf::from("~/fol/der"), &Path::new("/test")),
            PathBuf::from("/test/fol/der")
        )
    }

}
