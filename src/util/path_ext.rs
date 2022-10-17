use std::fs;
use std::path::Path;
use std::time;

/// An extension trait for [`Path`] types.
///
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
pub trait PathExt {
    fn metadata_modified(&self) -> Option<time::SystemTime>;

    fn newer_than<P>(&self, other: P) -> bool
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
}
