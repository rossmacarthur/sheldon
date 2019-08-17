use std::{
    fs,
    path::{Path, PathBuf},
    time,
};

/// An extension trait for [`Path`] types.
///
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
pub trait PathExt {
    fn metadata_modified(&self) -> Option<time::SystemTime>;
    fn newer_than(&self, other: &Path) -> bool;
}

pub trait PathBufExt {
    fn expand_tilde<P>(self, home: P) -> Self
    where
        P: AsRef<Path>;
    fn replace_home<P>(self, home: P) -> Self
    where
        P: AsRef<Path>;
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

impl PathBufExt for PathBuf {
    /// Expands the tilde in the path the given home.
    fn expand_tilde<P>(self, home: P) -> Self
    where
        P: AsRef<Path>,
    {
        if let Ok(path) = self.strip_prefix("~") {
            home.as_ref().join(path)
        } else {
            self
        }
    }

    /// Replaces the home directory in the path with a tilde.
    fn replace_home<P>(self, home: P) -> Self
    where
        P: AsRef<Path>,
    {
        if let Ok(path) = self.strip_prefix(home) {
            Path::new("~").join(path)
        } else {
            self
        }
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
        assert_eq!(PathBuf::from("/").expand_tilde("/test"), PathBuf::from("/"))
    }

    #[test]
    fn expand_tilde_with_folder_in_root() {
        assert_eq!(
            PathBuf::from("/fol/der").expand_tilde("/test"),
            PathBuf::from("/fol/der")
        )
    }

    #[test]
    fn expand_tilde_with_home() {
        assert_eq!(
            PathBuf::from("~/").expand_tilde("/test"),
            PathBuf::from("/test")
        )
    }

    #[test]
    fn expand_tilde_with_folder_in_home() {
        assert_eq!(
            PathBuf::from("~/fol/der").expand_tilde("/test"),
            PathBuf::from("/test/fol/der")
        )
    }

    #[test]
    fn replace_home_with_root() {
        assert_eq!(
            PathBuf::from("/not/home").replace_home("/test/home"),
            PathBuf::from("/not/home")
        )
    }

    #[test]
    fn replace_home_with_home() {
        assert_eq!(
            PathBuf::from("/test/home").replace_home("/test/home"),
            PathBuf::from("~")
        )
    }
}
