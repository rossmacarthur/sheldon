//! Utility traits and functions.

use std::ffi;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::result;
use std::time;

use anyhow::{Context as ResultExt, Error, Result};
use fs2::{lock_contended_error, FileExt};

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

/// Download a remote file.
pub fn download(url: &str, mut file: File) -> result::Result<(), curl::Error> {
    let mut easy = curl::easy::Easy::new();
    easy.fail_on_error(true)?; // -f
    easy.follow_location(true)?; // -L
    easy.url(url.as_ref())?;
    let mut transfer = easy.transfer();
    transfer.write_function(move |data| {
        match file.write_all(data) {
            Ok(()) => Ok(data.len()),
            Err(_) => Ok(0), // signals to cURL that the writing failed
        }
    })?;
    transfer.perform()?;
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////
// PathExt trait
////////////////////////////////////////////////////////////////////////////////

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

////////////////////////////////////////////////////////////////////////////////
// TempPath type
////////////////////////////////////////////////////////////////////////////////

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
            nuke_path(&path).ok();
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Mutex type
////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Mutex(File);

impl Mutex {
    /// Create a new `Mutex` at the given path and attempt to acquire it.
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

////////////////////////////////////////////////////////////////////////////////
// Git module
////////////////////////////////////////////////////////////////////////////////

pub mod git {
    use std::path::Path;

    use git2::{
        BranchType, Cred, CredentialType, Error, FetchOptions, Oid, RemoteCallbacks, Repository,
        ResetType,
    };
    use once_cell::sync::Lazy;
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

    static DEFAULT_REFSPECS: Lazy<Vec<String>> = Lazy::new(|| {
        vec_into![
            "refs/heads/*:refs/remotes/origin/*",
            "HEAD:refs/remotes/origin/HEAD"
        ]
    });

    /// Clone a Git repository.
    pub fn clone(url: &Url, dir: &Path) -> anyhow::Result<Repository> {
        with_fetch_options(|mut opts| {
            let repo = Repository::init(dir)?;
            repo.remote("origin", url.as_str())?
                .fetch(&DEFAULT_REFSPECS, Some(&mut opts), None)?;
            Ok(repo)
        })
        .with_context(s!("failed to git clone `{}`", url))
    }

    /// Fetch a Git repository.
    pub fn fetch(repo: &Repository) -> anyhow::Result<()> {
        with_fetch_options(|mut opts| {
            repo.find_remote("origin")
                .context("failed to find remote `origin`")?
                .fetch(&DEFAULT_REFSPECS, Some(&mut opts), None)?;
            Ok(())
        })
        .context("failed to git fetch")
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

    fn resolve_refname(repo: &Repository, refname: &str) -> Result<Oid, Error> {
        let ref_id = repo.refname_to_id(refname)?;
        let obj = repo.find_object(ref_id, None)?;
        let obj = obj.peel(git2::ObjectType::Commit)?;
        Ok(obj.id())
    }

    /// Get the *remote* HEAD as an object identifier.
    pub fn resolve_head(repo: &Repository) -> anyhow::Result<Oid> {
        resolve_refname(repo, "refs/remotes/origin/HEAD").context("failed to find remote HEAD")
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

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

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
