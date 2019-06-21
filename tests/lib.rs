use std::{fs, io, path::Path, process::Command};

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

trait Sheldon {
    fn sheldon<R>(root: R) -> Self
    where
        R: AsRef<Path>;
}

impl Sheldon for Command {
    fn sheldon<R>(root: R) -> Self
    where
        R: AsRef<Path>,
    {
        let mut command = Command::cargo_bin("sheldon").expect("`sheldon` binary in this crate");
        command
            .env("HOME", root.as_ref())
            .env("SHELDON_ROOT", root.as_ref())
            .arg("--verbose")
            .arg("--no-color");
        command
    }
}

fn git_status(directory: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .arg("status")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn lock_and_source_empty_config() -> Result<(), io::Error> {
    // Setup a temporary directory to operate in.
    let root = assert_fs::TempDir::new().unwrap();
    let config_file = root.child("plugins.toml");
    let lock_file = root.child("plugins.lock");

    // Given an empty config.
    config_file.touch().unwrap();

    // Run `sheldon lock` and check output.
    Command::sheldon(root.path())
        .arg("lock")
        .output()?
        .assert()
        .success()
        .stdout("")
        .stderr("[LOADED] ~/plugins.toml\n[LOCKED] ~/plugins.lock\n");

    // Verify the lock file contents.
    assert_eq!(
        fs::read_to_string(lock_file.path())?,
        format!(
            r#"version = "{version}"
home = "{root}"
root = "{root}"
config_file = "{config_file}"
lock_file = "{lock_file}"
plugins = []

[templates]
"#,
            version = env!("CARGO_PKG_VERSION"),
            root = root.path().display(),
            config_file = config_file.path().display(),
            lock_file = lock_file.path().display()
        )
    );

    // Run `sheldon source` and check output.
    Command::sheldon(root.path())
        .arg("source")
        .output()?
        .assert()
        .success()
        .stdout("")
        .stderr("[UNLOCKED] ~/plugins.lock\n");

    Ok(())
}

#[test]
fn lock_and_source_one_git_plugin() -> Result<(), io::Error> {
    // Setup a temporary directory to operate in.
    let root = assert_fs::TempDir::new().unwrap();
    let config_file = root.child("plugins.toml");
    let lock_file = root.child("plugins.lock");

    // Given a config with a single Git plugin.
    config_file
        .write_str("[plugins.test]\ngithub = 'rossmacarthur/sheldon-test'\n")
        .unwrap();

    // Run `sheldon lock` and check output.
    Command::sheldon(root.path())
        .arg("lock")
        .output()?
        .assert()
        .success()
        .stdout("")
        .stderr(
            r#"[LOADED] ~/plugins.toml
    [CLONED] https://github.com/rossmacarthur/sheldon-test
[LOCKED] ~/plugins.lock
"#,
        );

    // Check that sheldon-test was in fact downloaded.
    let directory = root.child("repositories/github.com/rossmacarthur/sheldon-test");
    directory.assert(predicate::path::is_dir());
    let filename = directory.child("test.plugin.zsh");
    filename.assert(predicate::path::is_file());
    assert_eq!(
        git_status(directory.path()),
        r#"On branch master
Your branch is up to date with 'origin/master'.

nothing to commit, working tree clean
"#,
    );

    // Verify the lock file contents.
    assert_eq!(
        fs::read_to_string(lock_file.path())?,
        format!(
            r#"version = "{version}"
home = "{root}"
root = "{root}"
config_file = "{config_file}"
lock_file = "{lock_file}"

[[plugins]]
name = "test"
directory = "{directory}"
filenames = ["{filename}"]
apply = ["source"]

[templates]
"#,
            version = env!("CARGO_PKG_VERSION"),
            root = root.path().display(),
            config_file = config_file.path().display(),
            lock_file = lock_file.path().display(),
            directory = directory.path().display(),
            filename = filename.path().display()
        )
    );

    // Run `sheldon source` and check output.
    Command::sheldon(root.path())
        .arg("source")
        .output()?
        .assert()
        .success()
        .stdout(format!("source \"{}\"\n", filename.path().display()))
        .stderr("[UNLOCKED] ~/plugins.lock\n  [RENDERED] test\n");

    Ok(())
}
