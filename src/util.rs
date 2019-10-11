use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time,
};

use fs2::{lock_contended_error, FileExt};

use crate::{Context, Result, ResultExt};

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

#[derive(Debug)]
pub struct FileMutex(File);

impl FileMutex {
    /// Create a new `FileMutex` at the given path and attempt to acquire it.
    pub fn acquire(ctx: &Context, path: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .open(path)
            .chain(s!("failed to open `{}`", path.display()))?;

        if let Err(e) = file.try_lock_exclusive() {
            let msg = s!("failed to acquire file lock `{}`", path.display());

            if e.raw_os_error() != lock_contended_error().raw_os_error() {
                return Err(e).chain(msg);
            } else {
                ctx.warning(
                    "Blocking",
                    &format!(
                        "waiting for file lock on {}",
                        ctx.replace_home(path).display()
                    ),
                );
                file.lock_exclusive().chain(msg)?;
            }
        }

        Ok(Self(file))
    }
}

impl Drop for FileMutex {
    fn drop(&mut self) {
        self.0.unlock().ok();
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

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
