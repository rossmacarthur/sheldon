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

            if e.raw_os_error() == lock_contended_error().raw_os_error() {
                ctx.warning(
                    "Blocking",
                    &format!(
                        "waiting for file lock on {}",
                        ctx.replace_home(path).display()
                    ),
                );
                file.lock_exclusive().chain(msg)?;
            } else {
                return Err(e).chain(msg);
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

pub mod git {
    use std::path::Path;

    use git2::{
        build::RepoBuilder, BranchType, Cred, CredentialType, Error, ErrorCode, FetchOptions, Oid,
        RemoteCallbacks, Repository, ResetType,
    };
    use url::Url;

    use crate::ResultExt;

    /// Call a function with generated fetch options.
    fn with_fetch_options<T, F>(f: F) -> crate::Result<T>
    where
        F: FnOnce(FetchOptions<'_>) -> crate::Result<T>,
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
    pub fn clone_or_open(url: &Url, directory: &Path) -> crate::Result<(bool, Repository)> {
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
                        Repository::open(directory)
                            .chain(s!("failed to open repository at `{}`", directory.display()))?
                    } else {
                        return Err(e).chain(s!("failed to git clone `{}`", url));
                    }
                }
            };
            Ok((cloned, repo))
        })
    }

    /// Checkout at repository at a particular revision.
    pub fn checkout(repo: &Repository, oid: Oid) -> crate::Result<()> {
        let obj = repo
            .find_object(oid, None)
            .chain(s!("failed to find `{}`", oid))?;
        repo.reset(&obj, ResetType::Hard, None)
            .chain(s!("failed to set HEAD to `{}`", oid))?;
        repo.checkout_tree(&obj, None)
            .chain(s!("failed to checkout `{}`", oid))
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
    pub fn resolve_branch(repo: &Repository, branch: &str) -> crate::Result<Oid> {
        repo.find_branch(&format!("origin/{}", branch), BranchType::Remote)
            .chain(s!("failed to find branch `{}`", branch))?
            .get()
            .target()
            .chain(s!("branch `{}` does not have a target", branch))
    }

    /// Resolve a revision to a object identifier.
    pub fn resolve_revision(repo: &Repository, revision: &str) -> crate::Result<Oid> {
        let obj = repo
            .revparse_single(revision)
            .chain(s!("failed to find revision `{}`", revision))?;
        Ok(match obj.as_tag() {
            Some(tag) => tag.target_id(),
            None => obj.id(),
        })
    }

    /// Resolve a tag to a object identifier.
    pub fn resolve_tag(repo: &Repository, tag: &str) -> crate::Result<Oid> {
        fn _resolve_tag(repo: &Repository, tag: &str) -> Result<Oid, Error> {
            let id = repo.refname_to_id(&format!("refs/tags/{}", tag))?;
            let obj = repo.find_object(id, None)?;
            let obj = obj.peel(git2::ObjectType::Commit)?;
            Ok(obj.id())
        }
        _resolve_tag(repo, tag).chain(s!("failed to find tag `{}`", tag))
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
