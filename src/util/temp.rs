use std::ffi;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::result;

use anyhow::Result;

/// Holds a temporary directory or file path that is removed when dropped.
pub struct TempPath {
    /// The temporary directory or file path.
    path: Option<PathBuf>,
}

impl TempPath {
    /// Create a new `TempPath` based on an original path, the temporary
    /// filename will be placed in the same directory with a deterministic name.
    ///
    /// # Errors
    ///
    /// If the temporary path already exists.
    pub fn new(original_path: &Path) -> result::Result<Self, PathBuf> {
        let mut path = original_path.parent().unwrap().to_path_buf();
        let mut file_name = ffi::OsString::from("~");
        file_name.push(original_path.file_name().unwrap());
        path.push(file_name);
        if path.exists() {
            Err(path)
        } else {
            Ok(Self::new_unchecked(path))
        }
    }

    /// Create a new `TempPath` based on an original path, if something exists
    /// at that temporary path is will be deleted.
    pub fn new_force(original_path: &Path) -> Result<Self> {
        match Self::new(original_path) {
            Ok(temp) => Ok(temp),
            Err(path) => {
                nuke_path(&path)?;
                Ok(Self::new_unchecked(path))
            }
        }
    }

    /// Create a new `TempPath` using the given temporary path.
    pub fn new_unchecked(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Access the underlying `Path`.
    pub fn path(&self) -> &Path {
        self.path.as_ref().unwrap()
    }

    /// Move the temporary path to a new location.
    pub fn rename(mut self, new_path: &Path) -> io::Result<()> {
        if let Err(err) = nuke_path(new_path) {
            if err.kind() != io::ErrorKind::NotFound {
                return Err(err);
            }
        };
        if let Some(path) = &self.path {
            fs::rename(path, new_path)?;
            // This is so that the Drop impl doesn't try delete a non-existent file.
            self.path = None;
        }
        Ok(())
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            nuke_path(path).ok();
        }
    }
}

/// Remove a file or directory.
fn nuke_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}
