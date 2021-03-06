//! Build information.
//!
//! Most of this information is generated by Cargo and the build script for this
//! crate.

use once_cell::sync::Lazy;

/// This is the name defined in the Cargo manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// This is the version defined in the Cargo manifest.
pub const CRATE_RELEASE: &str = env!("CARGO_PKG_VERSION");

/// This is the release with extra Git information if available.
pub static CRATE_VERSION: Lazy<String> = Lazy::new(|| {
    GIT.as_ref().map_or_else(
        || CRATE_RELEASE.to_string(),
        |git| {
            format!(
                "{} ({} {})",
                CRATE_RELEASE, git.commit_short_hash, git.commit_date
            )
        },
    )
});

/// This is the release with extra Git and Rustc information if available.
pub static CRATE_LONG_VERSION: Lazy<String> =
    Lazy::new(|| format!("{}\n{}", &*CRATE_VERSION, env!("RUSTC_VERSION_SUMMARY")));

/// This is a very verbose description of the crate version.
pub static CRATE_VERBOSE_VERSION: Lazy<String> = Lazy::new(|| {
    let (commit_hash, commit_date) = GIT
        .as_ref()
        .map(|git| (git.commit_hash, git.commit_date))
        .unwrap_or(("unknown", "unknown"));
    let mut v = CRATE_VERSION.clone();
    macro_rules! push {
        ($($arg:tt)*) => {v.push('\n'); v.push_str(&format!($($arg)+))};
    }
    push!("\nDetails:");
    push!("  binary: {}", CRATE_NAME);
    push!("  release: {}", CRATE_RELEASE);
    push!("  commit-hash: {}", commit_hash);
    push!("  commit-date: {}", commit_date);
    push!("  target: {}", env!("TARGET"));
    push!("\nCompiled with:");
    push!("  binary: {}", env!("RUSTC_VERSION_BINARY"));
    push!("  release: {}", env!("RUSTC_VERSION_RELEASE"));
    push!("  commit-hash: {}", env!("RUSTC_VERSION_COMMIT_HASH"));
    push!("  commit-date: {}", env!("RUSTC_VERSION_COMMIT_DATE"));
    push!("  host: {}", env!("RUSTC_VERSION_HOST"));
    v
});

struct Git<'a> {
    commit_date: &'a str,
    commit_hash: &'a str,
    commit_short_hash: &'a str,
}

static GIT: Lazy<Option<Git>> = Lazy::new(|| {
    match (
        option_env!("GIT_COMMIT_DATE"),
        option_env!("GIT_COMMIT_HASH"),
        option_env!("GIT_COMMIT_SHORT_HASH"),
    ) {
        (Some(commit_date), Some(commit_hash), Some(commit_short_hash)) => Some(Git {
            commit_date,
            commit_hash,
            commit_short_hash,
        }),
        (None, None, None) => None,
        vars => {
            panic!("unexpected Git information: {:?}", vars)
        }
    }
});
