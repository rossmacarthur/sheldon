use std::fmt;
use std::path::PathBuf;

use anyhow::{Context as ResultExt, Result};
use url::Url;

use crate::config::GitReference;
use crate::context::Context;
use crate::lock::source::LockedSource;
use crate::lock::LockMode;
use crate::util::git;
use crate::util::TempPath;

#[derive(Clone, Debug)]
pub enum GitCheckout {
    /// Checkout the latest of the default branch (HEAD).
    DefaultBranch,
    /// Checkout the tip of a branch.
    Branch(String),
    /// Checkout a specific revision.
    Rev(String),
    /// Checkout a tag.
    Tag(String),
}

/// Clones a Git repository and checks it out at a particular revision.
pub fn lock(ctx: &Context, dir: PathBuf, url: &Url, c: GitCheckout) -> Result<LockedSource> {
    match ctx.lock_mode() {
        LockMode::Normal => match git::open(&dir) {
            Ok(repo) => {
                if checkout(ctx, &repo, url, c.clone()).is_err() {
                    git::fetch(&repo)?;
                    checkout(ctx, &repo, url, c)?;
                }
                Ok(LockedSource { dir, file: None })
            }
            Err(_) => install(ctx, dir, url, c),
        },
        LockMode::Update => match git::open(&dir) {
            Ok(repo) => {
                git::fetch(&repo)?;
                checkout(ctx, &repo, url, c)?;
                Ok(LockedSource { dir, file: None })
            }
            Err(_) => install(ctx, dir, url, c),
        },
        LockMode::Reinstall => install(ctx, dir, url, c),
    }
}

/// Checks if a repository is correctly checked out, if not checks it out.
fn checkout(
    ctx: &Context,
    repo: &git2::Repository,
    url: &Url,
    checkout: GitCheckout,
) -> Result<()> {
    let current_oid = repo.head()?.target().context("current HEAD as no target")?;
    let expected_oid = checkout.resolve(repo)?;
    if current_oid == expected_oid {
        ctx.log_status("Checked", &format!("{url}{checkout}"));
    } else {
        git::checkout(repo, expected_oid)?;
        git::submodule_update(repo).context("failed to recursively update")?;
        ctx.log_status(
            "Updated",
            &format!(
                "{}{} ({} to {})",
                url,
                checkout,
                &current_oid.to_string()[..7],
                &expected_oid.to_string()[..7]
            ),
        );
    }
    Ok(())
}

fn install(ctx: &Context, dir: PathBuf, url: &Url, checkout: GitCheckout) -> Result<LockedSource> {
    let temp_dir =
        TempPath::new_force(&dir).context("failed to prepare temporary clone directory")?;
    {
        let repo = git::clone(url, temp_dir.path())?;
        git::checkout(&repo, checkout.resolve(&repo)?)?;
        git::submodule_update(&repo).context("failed to recursively update")?;
    } // `repo` must be dropped before renaming the directory
    temp_dir
        .rename(&dir)
        .context("failed to rename temporary clone directory")?;
    ctx.log_status("Cloned", &format!("{url}{checkout}"));
    Ok(LockedSource { dir, file: None })
}

impl fmt::Display for GitCheckout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefaultBranch => write!(f, ""),
            Self::Branch(s) | Self::Rev(s) | Self::Tag(s) => write!(f, "@{s}"),
        }
    }
}

impl From<Option<GitReference>> for GitCheckout {
    fn from(reference: Option<GitReference>) -> Self {
        match reference {
            None => Self::DefaultBranch,
            Some(GitReference::Branch(s)) => Self::Branch(s),
            Some(GitReference::Rev(s)) => Self::Rev(s),
            Some(GitReference::Tag(s)) => Self::Tag(s),
        }
    }
}

impl GitCheckout {
    /// Resolve `GitCheckout` to a Git object identifier.
    fn resolve(&self, repo: &git2::Repository) -> Result<git2::Oid> {
        match self {
            Self::DefaultBranch => git::resolve_head(repo),
            Self::Branch(s) => git::resolve_branch(repo, s),
            Self::Rev(s) => git::resolve_rev(repo, s),
            Self::Tag(s) => git::resolve_tag(repo, s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::process::Command;
    use std::{fs, thread, time};

    #[test]
    fn git_checkout_to_string() {
        assert_eq!(
            GitCheckout::Branch("feature".to_string()).to_string(),
            "@feature"
        );
        assert_eq!(
            GitCheckout::Rev("ad149784a".to_string()).to_string(),
            "@ad149784a"
        );
        assert_eq!(GitCheckout::Tag("0.2.3".to_string()).to_string(), "@0.2.3");
    }

    #[test]
    fn git_checkout_resolve_branch() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let checkout = GitCheckout::Branch("feature".to_string());
        let oid = checkout.resolve(&repo).expect("lock git checkout");
        assert_eq!(oid.to_string(), "09ead574b20bb573ae0a53c1a5c546181cfa41c8");

        let checkout = GitCheckout::Branch("not-a-branch".to_string());
        let error = checkout.resolve(&repo).unwrap_err();
        assert_eq!(error.to_string(), "failed to find branch `not-a-branch`");
    }

    #[test]
    fn git_checkout_resolve_rev() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let checkout = GitCheckout::Rev("ad149784a".to_string());
        let oid = checkout.resolve(&repo).unwrap();
        assert_eq!(oid.to_string(), "ad149784a1538291f2477fb774eeeed4f4d29e45");

        let checkout = GitCheckout::Rev("2c4ed7710".to_string());
        let error = checkout.resolve(&repo).unwrap_err();
        assert_eq!(error.to_string(), "failed to find revision `2c4ed7710`");
    }

    #[test]
    fn git_checkout_resolve_tag() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let checkout = GitCheckout::Tag("v0.1.0".to_string());
        let oid = checkout.resolve(&repo).unwrap();
        assert_eq!(oid.to_string(), "be8fde277e76f35efbe46848fb352cee68549962");

        let checkout = GitCheckout::Tag("v0.2.0".to_string());
        let error = checkout.resolve(&repo).unwrap_err();
        assert_eq!(error.to_string(), "failed to find tag `v0.2.0`");
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

    #[test]
    fn lock_git_and_reinstall() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let mut ctx = Context::testing(dir);
        let url = Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap();

        let locked = lock(&ctx, dir.to_path_buf(), &url, GitCheckout::DefaultBranch).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(dir).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );

        let modified = fs::metadata(dir).unwrap().modified().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        ctx.lock_mode = Some(LockMode::Reinstall);
        let locked = lock(&ctx, dir.to_path_buf(), &url, GitCheckout::DefaultBranch).unwrap();
        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(dir).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );
        assert!(fs::metadata(dir).unwrap().modified().unwrap() > modified);
    }

    #[test]
    fn lock_git_https_with_checkout() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();

        let locked = lock(
            &Context::testing(dir),
            dir.to_path_buf(),
            &Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
            GitCheckout::Rev("ad149784a1538291f2477fb774eeeed4f4d29e45".to_string()),
        )
        .unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(dir).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(
            head.target().unwrap().to_string(),
            "ad149784a1538291f2477fb774eeeed4f4d29e45"
        );
    }

    #[test]
    #[ignore]
    fn lock_git_git_with_checkout() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();

        let locked = lock(
            &Context::testing(dir),
            dir.to_path_buf(),
            &Url::parse("git://github.com/rossmacarthur/sheldon-test").unwrap(),
            GitCheckout::Rev("ad149784a1538291f2477fb774eeeed4f4d29e45".to_string()),
        )
        .unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(dir).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(
            head.target().unwrap().to_string(),
            "ad149784a1538291f2477fb774eeeed4f4d29e45"
        );
    }
}
