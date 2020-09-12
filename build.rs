use std::env;

use anyhow::{Context, Result};
use chrono::NaiveDateTime as DateTime;

fn git_stuff() -> Result<()> {
    let dir = env::var("CARGO_MANIFEST_DIR")?;
    let repo = git2::Repository::open(dir)?;
    let git_describe = repo
        .describe(&git2::DescribeOptions::new().describe_tags())?
        .format(Some(
            &git2::DescribeFormatOptions::new().dirty_suffix("-dirty"),
        ))?;
    let commit = repo.head()?.peel_to_commit()?;
    let commit_date = DateTime::from_timestamp_opt(commit.time().seconds(), 0)
        .context("invalid timestamp")?
        .format("%Y-%m-%d");
    println!("cargo:rustc-env=GIT_DESCRIBE={}", git_describe);
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit.id());
    println!("cargo:rustc-env=GIT_COMMIT_DATE={}", commit_date);
    Ok(())
}

fn main() -> Result<()> {
    // Pass through the target that we are building.
    println!("cargo:rustc-env=TARGET={}", env::var("TARGET")?);

    // Try get Git information but ignore if we failed.
    git_stuff().ok();

    Ok(())
}
