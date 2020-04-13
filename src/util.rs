//! Utility traits and functions.

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time,
};

use anyhow::{Context as ResultExt, Result};
use fs2::{lock_contended_error, FileExt};

use crate::context::{Context, SettingsExt};

/// An extension trait for [`Path`] types.
///
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
pub trait PathExt {
    fn metadata_modified(&self) -> Option<time::SystemTime>;

    fn newer_than<P>(&self, other: P) -> bool
    where
        P: AsRef<Path>;

    fn expand_tilde<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Path>;

    fn replace_home<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Path>;
}

impl PathExt for Path {
    /// Returns the modified time of the file if available.
    fn metadata_modified(&self) -> Option<time::SystemTime> {
        fs::metadata(&self).and_then(|m| m.modified()).ok()
    }

    /// Returns whether the file at this path is newer than the file at the
    /// given one. If either file does not exist, this method returns `false`.
    fn newer_than<P>(&self, other: P) -> bool
    where
        P: AsRef<Path>,
    {
        match (self.metadata_modified(), other.as_ref().metadata_modified()) {
            (Some(self_time), Some(other_time)) => self_time > other_time,
            _ => false,
        }
    }

    /// Expands the tilde in the path with the given home directory.
    fn expand_tilde<P>(&self, home: P) -> PathBuf
    where
        P: AsRef<Path>,
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
        P: AsRef<Path>,
    {
        if let Ok(path) = self.strip_prefix(home) {
            Self::new("~").join(path)
        } else {
            self.to_path_buf()
        }
    }
}

#[derive(Debug)]
pub struct Mutex(File);

impl Mutex {
    /// Create a new `FileMutex` at the given path and attempt to acquire it.
    pub fn acquire(ctx: &Context, path: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .open(path)
            .with_context(s!("failed to open `{}`", path.display()))?;

        if let Err(e) = file.try_lock_exclusive() {
            let msg = s!("failed to acquire file lock `{}`", path.display());

            if e.raw_os_error() == lock_contended_error().raw_os_error() {
                warning!(
                    ctx,
                    "Blocking",
                    &format!(
                        "waiting for file lock on {}",
                        ctx.replace_home(path).display()
                    )
                );
                file.lock_exclusive().with_context(msg)?;
            } else {
                return Err(e).with_context(msg);
            }
        }

        Ok(Self(file))
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        self.0.unlock().ok();
    }
}

pub mod git {
    use std::path::Path;

    use git2::{
        build::RepoBuilder, BranchType, Cred, CredentialType, Error, ErrorCode, FetchOptions, Oid,
        RemoteCallbacks, Repository, ResetType,
    };
    use url::Url;

    use anyhow::Context as ResultExt;

    /// Call a function with generated fetch options.
    fn with_fetch_options<T, F>(f: F) -> anyhow::Result<T>
    where
        F: FnOnce(FetchOptions<'_>) -> anyhow::Result<T>,
    {
        let mut rcb = RemoteCallbacks::new();
        rcb.credentials(|_, username, allowed| {
            if allowed.contains(CredentialType::SSH_KEY) {
                if let Some(username) = username {
                    return Cred::ssh_key_from_agent(username);
                }
            }
            if allowed.contains(CredentialType::DEFAULT) {
                return Cred::default();
            }
            Err(Error::from_str(
                "remote authentication required but none available",
            ))
        });
        let mut opts = FetchOptions::new();
        opts.remote_callbacks(rcb);
        f(opts)
    }

    /// Clone or open a Git repository.
    pub fn clone_or_open(url: &Url, directory: &Path) -> anyhow::Result<(bool, Repository)> {
        with_fetch_options(|opts| {
            let mut cloned = false;
            let repo = match RepoBuilder::new()
                .fetch_options(opts)
                .clone(url.as_str(), directory)
            {
                Ok(repo) => {
                    cloned = true;
                    repo
                }
                Err(e) => {
                    if e.code() == ErrorCode::Exists {
                        Repository::open(directory).with_context(s!(
                            "failed to open repository at `{}`",
                            directory.display()
                        ))?
                    } else {
                        return Err(e).with_context(s!("failed to git clone `{}`", url));
                    }
                }
            };
            Ok((cloned, repo))
        })
    }

    /// Checkout at repository at a particular revision.
    pub fn checkout(repo: &Repository, oid: Oid) -> anyhow::Result<()> {
        let obj = repo
            .find_object(oid, None)
            .with_context(s!("failed to find `{}`", oid))?;
        repo.reset(&obj, ResetType::Hard, None)
            .with_context(s!("failed to set HEAD to `{}`", oid))?;
        repo.checkout_tree(&obj, None)
            .with_context(s!("failed to checkout `{}`", oid))
    }

    /// Recursively update Git submodules.
    pub fn submodule_update(repo: &Repository) -> Result<(), Error> {
        fn _submodule_update(repo: &Repository, todo: &mut Vec<Repository>) -> Result<(), Error> {
            for mut submodule in repo.submodules()? {
                submodule.update(true, None)?;
                todo.push(submodule.open()?);
            }
            Ok(())
        }
        let mut repos = Vec::new();
        _submodule_update(&repo, &mut repos)?;
        while let Some(repo) = repos.pop() {
            _submodule_update(&repo, &mut repos)?;
        }
        Ok(())
    }

    /// Resolve a branch to a object identifier.
    pub fn resolve_branch(repo: &Repository, branch: &str) -> anyhow::Result<Oid> {
        repo.find_branch(&format!("origin/{}", branch), BranchType::Remote)
            .with_context(s!("failed to find branch `{}`", branch))?
            .get()
            .target()
            .with_context(s!("branch `{}` does not have a target", branch))
    }

    /// Resolve a revision to a object identifier.
    pub fn resolve_revision(repo: &Repository, revision: &str) -> anyhow::Result<Oid> {
        let obj = repo
            .revparse_single(revision)
            .with_context(s!("failed to find revision `{}`", revision))?;
        Ok(match obj.as_tag() {
            Some(tag) => tag.target_id(),
            None => obj.id(),
        })
    }

    /// Resolve a tag to a object identifier.
    pub fn resolve_tag(repo: &Repository, tag: &str) -> anyhow::Result<Oid> {
        fn _resolve_tag(repo: &Repository, tag: &str) -> Result<Oid, Error> {
            let id = repo.refname_to_id(&format!("refs/tags/{}", tag))?;
            let obj = repo.find_object(id, None)?;
            let obj = obj.peel(git2::ObjectType::Commit)?;
            Ok(obj.id())
        }
        _resolve_tag(repo, tag).with_context(s!("failed to find tag `{}`", tag))
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
