//! Git helpers.

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

    // Try to auto-detect the proxy from the git configuration so that
    // Sheldon can be used behind a proxy.
    let mut proxy_opts = git2::ProxyOptions::new();
    proxy_opts.auto();

    let mut opts = FetchOptions::new();
    opts.remote_callbacks(rcb);
    opts.proxy_options(proxy_opts);
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
        "+refs/heads/*:refs/remotes/origin/*",
        "+HEAD:refs/remotes/origin/HEAD"
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
    _submodule_update(repo, &mut repos)?;
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
