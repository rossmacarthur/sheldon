//! Utility traits and functions.

use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process, time,
};

use anyhow::{Context as ResultExt, Error, Result};
use fs2::{lock_contended_error, FileExt};
use url::Url;

use crate::context::{Context, SettingsExt};

/// Returns the underlying error kind for the given error.
pub fn underlying_io_error_kind(error: &Error) -> Option<io::ErrorKind> {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<io::Error>() {
            return Some(io_error.kind());
        }
    }
    None
}

/// Remove a file or directory.
fn nuke_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Download a remote file and handle status code errors.
pub fn download(url: Url) -> reqwest::Result<reqwest::blocking::Response> {
    Ok(reqwest::blocking::get(url)?.error_for_status()?)
}

/////////////////////////////////////////////////////////////////////////
// PathExt trait
/////////////////////////////////////////////////////////////////////////

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

/////////////////////////////////////////////////////////////////////////
// TempPath type
/////////////////////////////////////////////////////////////////////////

/// Holds a temporary directory or file path that is removed when dropped.
pub struct TempPath {
    /// The temporary directory or file path.
    pub path: Option<PathBuf>,
}

impl TempPath {
    /// Create a new `TempPath`.
    pub fn new(original_path: &Path) -> Self {
        let mut path = original_path.parent().unwrap().to_path_buf();
        let mut file_name = original_path.file_stem().unwrap().to_os_string();
        file_name.push(format!("-tmp-{}", process::id()));
        if let Some(ext) = original_path.extension() {
            file_name.push(".");
            file_name.push(ext);
        }
        path.push(file_name);
        Self { path: Some(path) }
    }

    /// Access the underlying `Path`.
    pub fn path(&self) -> &Path {
        self.path.as_ref().unwrap()
    }

    /// Copy the contents of a stream to this `TempPath`.
    pub fn write<R>(&mut self, mut reader: &mut R) -> io::Result<()>
    where
        R: io::Read,
    {
        let mut file = fs::File::create(self.path())?;
        io::copy(&mut reader, &mut file)?;
        Ok(())
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
            nuke_path(&path).ok();
        }
    }
}

/////////////////////////////////////////////////////////////////////////
// Mutex type
/////////////////////////////////////////////////////////////////////////

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

/////////////////////////////////////////////////////////////////////////
// Git module
/////////////////////////////////////////////////////////////////////////

pub mod git {
    use std::path::Path;

    use git2::{
        build::RepoBuilder, BranchType, Cred, CredentialType, Error, FetchOptions, Oid,
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

    /// Open a Git repository.
    pub fn open(dir: &Path) -> anyhow::Result<Repository> {
        let repo = Repository::open(dir)
            .with_context(s!("failed to open repository at `{}`", dir.display()))?;
        Ok(repo)
    }

    /// Clone a Git repository.
    pub fn clone(url: &Url, dir: &Path) -> anyhow::Result<Repository> {
        with_fetch_options(|opts| {
            let repo = RepoBuilder::new()
                .fetch_options(opts)
                .clone(url.as_str(), dir)
                .with_context(s!("failed to git clone `{}`", url))?;
            Ok(repo)
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
    pub fn resolve_rev(repo: &Repository, rev: &str) -> anyhow::Result<Oid> {
        let obj = repo
            .revparse_single(rev)
            .with_context(s!("failed to find revision `{}`", rev))?;
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
