mod git;
mod local;
mod remote;

use std::fmt;
use std::path::PathBuf;

use anyhow::{Context as ResultExt, Result};

use crate::config::Source;
use crate::context::Context;
use crate::lock::source::git::GitCheckout;

/// A locked `Source`.
#[derive(Clone, Debug, PartialEq)]
pub struct LockedSource {
    /// The clone or download directory.
    pub dir: PathBuf,
    /// The downloaded file.
    pub file: Option<PathBuf>,
}

// Install a source.
pub fn lock(ctx: &Context, src: Source) -> Result<LockedSource> {
    match src {
        Source::Git { url, reference } => {
            let mut dir = ctx.clone_dir().to_path_buf();
            dir.push(
                url.host_str()
                    .with_context(s!("URL `{}` has no host", url))?,
            );
            dir.push(url.path().trim_start_matches('/'));
            git::lock(ctx, dir, &url, reference.into())
        }

        Source::Remote { url } => {
            let mut dir = ctx.download_dir().to_path_buf();
            dir.push(
                url.host_str()
                    .with_context(s!("URL `{}` has no host", url))?,
            );

            let segments: Vec<_> = url
                .path_segments()
                .with_context(s!("URL `{}` is cannot-be-a-base", url))?
                .collect();
            let (base, rest) = segments.split_last().unwrap();
            let base = if base.is_empty() { "index" } else { *base };
            dir.push(rest.iter().collect::<PathBuf>());
            let file = dir.join(base);

            remote::lock(ctx, dir, file, &url)
        }

        Source::Local { dir } => local::lock(ctx, dir),
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git { url, reference } => {
                let checkout: GitCheckout = reference.clone().into();
                write!(f, "{}{}", url, checkout)
            }
            Self::Remote { url, .. } => write!(f, "{}", url),
            Self::Local { dir } => write!(f, "{}", dir.display()),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use url::Url;

    use crate::config::GitReference;

    #[test]
    fn source_to_string() {
        assert_eq!(
            Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.3.0".to_string())),
            }
            .to_string(),
            "https://github.com/rossmacarthur/sheldon-test@v0.3.0"
        );
        assert_eq!(
            Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: None,
            }
            .to_string(),
            "https://github.com/rossmacarthur/sheldon-test"
        );
        assert_eq!(
            Source::Remote {
                url: Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT")
                    .unwrap(),
            }
            .to_string(),
            "https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT"
        );
        assert_eq!(
            Source::Local {
                dir: PathBuf::from("~/plugins")
            }
            .to_string(),
            "~/plugins"
        );
    }

    #[test]
    fn lock_with_git() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);

        let source = Source::Git {
            url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
            reference: None,
        };
        let locked = lock(&ctx, source).unwrap();

        assert_eq!(
            locked,
            LockedSource {
                dir: dir.join("repos/github.com/rossmacarthur/sheldon-test"),
                file: None,
            }
        );
    }

    #[test]
    fn lock_with_remote() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);

        let source = Source::Remote {
            url: Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT")
                .unwrap(),
        };
        let locked = lock(&ctx, source).unwrap();

        assert_eq!(
            locked.dir,
            dir.join("downloads/github.com/rossmacarthur/sheldon/raw/0.3.0")
        );
        assert_eq!(
            locked.file,
            Some(dir.join("downloads/github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT"))
        );
    }
}
