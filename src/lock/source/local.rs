use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::context::Context;
use crate::lock::source::LockedSource;

/// Checks that a Local source directory exists.
pub fn lock(ctx: &Context, dir: PathBuf) -> Result<LockedSource> {
    let dir = ctx.expand_tilde(dir);

    if dir.exists() && dir.is_dir() {
        ctx.log_status("Checked", dir.as_path());
        Ok(LockedSource { dir, file: None })
    } else if let Ok(walker) = globwalk::glob(dir.to_string_lossy()) {
        let mut directories: Vec<_> = walker
            .filter_map(|result| match result {
                Ok(entry) if entry.path().is_dir() => Some(entry.into_path()),
                _ => None,
            })
            .collect();

        if directories.len() == 1 {
            let dir = directories.remove(0);
            ctx.log_status("Checked", dir.as_path());
            Ok(LockedSource { dir, file: None })
        } else {
            Err(anyhow!(
                "`{}` matches {} directories",
                dir.display(),
                directories.len()
            ))
        }
    } else {
        Err(anyhow!("`{}` is not a dir", dir.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::process::Command;

    #[test]
    fn lock_local() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let _repo = git_clone_sheldon_test(&temp);

        let locked = lock(&Context::testing(dir), dir.to_path_buf()).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
    }

    fn git_clone_sheldon_test(temp: &tempfile::TempDir) -> git2::Repository {
        let dir = temp.path();
        Command::new("git")
            .arg("clone")
            .arg("https://github.com/rossmacarthur/sheldon-test")
            .arg(dir)
            .output()
            .expect("git clone rossmacarthur/sheldon-test");
        git2::Repository::open(dir).expect("open sheldon-test git repository")
    }
}
