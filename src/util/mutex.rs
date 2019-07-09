use std::{
    fs::{self, File},
    path::Path,
};

use fs2::{lock_contended_error, FileExt};

use crate::{Context, Result, ResultExt};

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
