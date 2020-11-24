use std::env;
use std::io;
use std::path::PathBuf;
use std::process;

use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;

/// The directory containing the `Cargo.toml` file.
static CARGO_MANIFEST_DIR: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()));

/// Nicely format an error message for when the subprocess didn't exit
/// successfully.
pub fn format_error_msg(cmd: &process::Command, output: process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut msg = format!(
        "subprocess didn't exit successfully `{:?}` ({})",
        cmd, output.status
    );
    if !stdout.trim().is_empty() {
        msg.push_str(&format!("\n--- stdout\n{}", stdout));
    }
    if !stderr.trim().is_empty() {
        msg.push_str(&format!("\n--- stderr\n{}", stderr));
    }
    msg
}

/// Whether underlying error kind for the given error is
/// `io::ErrorKind::NotFound`.
pub fn is_io_not_found(error: &anyhow::Error) -> bool {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<io::Error>() {
            return io_error.kind() == io::ErrorKind::NotFound;
        }
    }
    false
}

trait CommandExt {
    /// Run the command return the standard output as a UTF-8 string.
    fn output_text(&mut self) -> Result<String>;
}

impl CommandExt for process::Command {
    /// Run the command return the standard output as a UTF-8 string.
    fn output_text(&mut self) -> Result<String> {
        let output = self
            .output()
            .with_context(|| format!("could not execute subprocess: `{:?}`", self))?;
        if !output.status.success() {
            bail!(format_error_msg(self, output));
        }
        Ok(String::from_utf8(output.stdout).context("failed to parse stdout")?)
    }
}

/// Simple macro to run a Git subcommand and set the result as a rustc
/// environment variable.
///
/// Note:
/// - The Cargo manifest directory is passed as the Git directory.
/// - If the Git subcommand is not available then this macro will `return
///   Ok(())`.
macro_rules! print_git_env {
    ($key:expr, $cmd:expr) => {{
        let mut split = $cmd.split_whitespace();
        let value = match process::Command::new(split.next().unwrap())
            .arg("-C")
            .arg(&*CARGO_MANIFEST_DIR)
            .args(split)
            .output_text()
        {
            Ok(text) => text.trim().to_string(),
            Err(err) if is_io_not_found(&err) => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        println!("cargo:rustc-env={}={}", $key, value);
    }};
}

/// Fetch Git info and set as rustc environment variables.
///
/// If the Git subcommand is missing or the `.git` directory does not exist then
/// no errors will be produced.
fn print_git_envs() -> Result<()> {
    let git_dir = CARGO_MANIFEST_DIR.join(".git");
    if !git_dir.exists() {
        return Ok(());
    }
    print_git_env!(
        "GIT_COMMIT_DATE",
        "git log -1 --no-show-signature --date=short --format=%cd"
    );
    print_git_env!("GIT_COMMIT_HASH", "git rev-parse HEAD");
    print_git_env!("GIT_COMMIT_SHORT_HASH", "git rev-parse --short=9 HEAD");
    Ok(())
}

/// Fetch rustc info and set as rustc environment variables.
fn print_rustc_envs() -> Result<()> {
    let text = process::Command::new(env::var("RUSTC")?)
        .arg("--verbose")
        .arg("--version")
        .output_text()?;
    let mut lines = text.lines();
    println!(
        "cargo:rustc-env=RUSTC_VERSION_SUMMARY={}",
        lines.next().unwrap()
    );
    for line in lines {
        let mut split = line.splitn(2, ": ");
        let key = split.next().unwrap();
        let value = split.next().unwrap();
        println!(
            "cargo:rustc-env=RUSTC_VERSION_{}={}",
            key.replace('-', "_").replace(' ', "_").to_uppercase(),
            value,
        )
    }
    Ok(())
}

fn main() -> Result<()> {
    print_git_envs().context("failed to fetch Git information")?;
    print_rustc_envs().context("failed to fetch rustc information")?;
    println!("cargo:rustc-env=TARGET={}", env::var("TARGET")?);
    Ok(())
}
