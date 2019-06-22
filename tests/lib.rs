use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use assert_cmd::prelude::*;
use assert_fs::{fixture::TempDir, prelude::*};
use handlebars::Handlebars;
use maplit::hashmap;
use predicates::prelude::*;

/////////////////////////////////////////////////////////////////////////
// Utilities
/////////////////////////////////////////////////////////////////////////

fn sheldon<R>(root: R) -> Command
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

pub fn git_status(directory: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .arg("status")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub struct TestCase {
    path: PathBuf,
    pub root: TempDir,
}

impl TestCase {
    pub fn new(name: &'static str) -> io::Result<Self> {
        let path: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        Ok(Self {
            path: path.join("tests/cases").join(name),
            root: TempDir::new().unwrap(),
        })
    }

    pub fn run(&self) -> io::Result<()> {
        let config_path_name = "plugins.toml";
        let lock_path_name = "plugins.lock";
        let config_path = self.root.path().join(config_path_name);
        let lock_path = self.root.path().join(lock_path_name);

        let data = hashmap! {
            "version" => env!("CARGO_PKG_VERSION"),
            "root" => self.root.path().to_str().unwrap(),
            "config_path" => config_path.to_str().unwrap(),
            "lock_path" => lock_path.to_str().unwrap(),
        };

        let mut handlebars = Handlebars::new();
        for (name, dest) in &[
            (config_path_name, &config_path),
            (lock_path_name, &lock_path),
        ] {
            let source = self.path.join(name);
            handlebars.register_template_file(name, source).unwrap();
            let file = fs::File::create(dest)?;
            handlebars.render_to_write(name, &data, file).unwrap();
        }

        for command in &["lock", "source"] {
            sheldon(&self.root.path())
                .arg(command)
                .assert()
                .success()
                .stdout(
                    handlebars
                        .render_template(
                            &fs::read_to_string(self.path.join(command).with_extension("stdout"))?,
                            &data,
                        )
                        .unwrap(),
                )
                .stderr(
                    handlebars
                        .render_template(
                            &fs::read_to_string(self.path.join(command).with_extension("stderr"))?,
                            &data,
                        )
                        .unwrap(),
                );
        }

        Ok(())
    }
}

/////////////////////////////////////////////////////////////////////////
// Tests
/////////////////////////////////////////////////////////////////////////

#[test]
fn empty() -> io::Result<()> {
    let test_case = TestCase::new("empty")?;
    test_case.run()
}

#[test]
fn git() -> io::Result<()> {
    let case = TestCase::new("git")?;
    case.run()?;

    // Check that sheldon-test was in fact downloaded.
    let directory = case
        .root
        .child("repositories/github.com/rossmacarthur/sheldon-test");
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

    Ok(())
}

#[test]
fn git_branch() -> io::Result<()> {
    let case = TestCase::new("git_branch")?;
    case.run()?;

    // Check that sheldon-test@feature was in fact downloaded.
    let directory = case
        .root
        .child("repositories/github.com/rossmacarthur/sheldon-test");
    directory.assert(predicate::path::is_dir());
    let filename = directory.child("test.plugin.zsh");
    filename.assert(predicate::path::is_file());
    assert_eq!(
        git_status(directory.path()),
        r#"On branch master
Your branch is ahead of 'origin/master' by 1 commit.
  (use "git push" to publish your local commits)

nothing to commit, working tree clean
"#,
    );

    Ok(())
}

#[test]
fn git_tag() -> io::Result<()> {
    let case = TestCase::new("git_tag")?;
    case.run()?;

    // Check that sheldon-test@v0.1.0 was in fact downloaded.
    let directory = case
        .root
        .child("repositories/github.com/rossmacarthur/sheldon-test");
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

    Ok(())
}
