use std::fs;
use std::path::{Path, PathBuf};
use std::time;

/// An extension trait for [`Path`] types.
///
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
pub trait PathExt {
    fn metadata_modified(&self) -> Option<time::SystemTime>;

    fn newer_than<P>(&self, other: P) -> bool
    where
        P: AsRef<Self>;

    fn expand_tilde<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Self>;

    fn replace_home<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Self>;
}

impl PathExt for Path {
    /// Returns the modified time of the file if available.
    fn metadata_modified(&self) -> Option<time::SystemTime> {
        fs::metadata(self).and_then(|m| m.modified()).ok()
    }

    /// Returns whether the file at this path is newer than the file at the
    /// given one. If either file does not exist, this method returns `false`.
    fn newer_than<P>(&self, other: P) -> bool
    where
        P: AsRef<Self>,
    {
        match (self.metadata_modified(), other.as_ref().metadata_modified()) {
            (Some(self_time), Some(other_time)) => self_time > other_time,
            _ => false,
        }
    }

    /// Expands the tilde in the path with the given home directory.
    fn expand_tilde<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Self>,
    {
        if let Ok(path) = self.strip_prefix("~") {
            home.as_ref().join(path)
        } else {
            self.to_path_buf()
        }
    }

    /// Replaces the home directory in the path with a tilde.
    fn replace_home<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Self>,
    {
        if let Ok(path) = self.strip_prefix(home) {
            Self::new("~").join(path)
        } else {
            self.to_path_buf()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn path_buf_expand_tilde_with_root() {
        assert_eq!(PathBuf::from("/").expand_tilde("/test"), PathBuf::from("/"))
    }

    #[test]
    fn path_buf_expand_tilde_with_folder_in_root() {
        assert_eq!(
            PathBuf::from("/fol/der").expand_tilde("/test"),
            PathBuf::from("/fol/der")
        )
    }

    #[test]
    fn path_buf_expand_tilde_with_home() {
        assert_eq!(
            PathBuf::from("~/").expand_tilde("/test"),
            PathBuf::from("/test")
        )
    }

    #[test]
    fn path_buf_expand_tilde_with_folder_in_home() {
        assert_eq!(
            PathBuf::from("~/fol/der").expand_tilde("/test"),
            PathBuf::from("/test/fol/der")
        )
    }

    #[test]
    fn path_buf_replace_home_with_root() {
        assert_eq!(
            PathBuf::from("/not/home").replace_home("/test/home"),
            PathBuf::from("/not/home")
        )
    }

    #[test]
    fn path_buf_replace_home_with_home() {
        assert_eq!(
            PathBuf::from("/test/home").replace_home("/test/home"),
            PathBuf::from("~")
        )
    }
}
