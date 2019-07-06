use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use assert_cmd::prelude::*;

use handlebars::Handlebars;
use maplit::hashmap;

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
    pub path: PathBuf,
    pub root: tempfile::TempDir,
    pub config_file: PathBuf,
    pub lock_file: PathBuf,
    pub handlebars: Handlebars,
    pub data: HashMap<&'static str, String>,
}

impl TestCase {
    pub fn new(name: &'static str) -> io::Result<Self> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases")
            .join(name);
        let root = tempfile::tempdir()?;
        let config_file = root.path().join("plugins.toml");
        let lock_file = root.path().join("plugins.lock");
        let data = hashmap! {
            "version" => env!("CARGO_PKG_VERSION").into(),
            "root" => root.path().to_string_lossy().into(),
            "config_file" => config_file.to_string_lossy().into(),
            "lock_file" => lock_file.to_string_lossy().into(),
        };
        let handlebars = Handlebars::new();
        let case = Self {
            path,
            root,
            config_file,
            lock_file,
            handlebars,
            data,
        };

        // Render the plugins.toml
        case.render_file(&case.path.join("plugins.toml"), &case.config_file);

        Ok(case)
    }

    pub fn render(&self, path: &Path) -> String {
        self.handlebars
            .render_template(
                &fs::read_to_string(path).expect("failed to read string from file"),
                &self.data,
            )
            .expect("failed to render string")
    }

    pub fn render_file(&self, source: &Path, destination: &Path) {
        let file = fs::File::create(destination).expect("failed to create file");
        self.handlebars
            .render_template_to_write(&fs::read_to_string(source).unwrap(), &self.data, file)
            .expect("failed to render file")
    }

    pub fn stdout(&self, command: &str) -> String {
        self.render(&self.path.join(command).with_extension("stdout"))
    }

    pub fn stderr(&self, command: &str) -> String {
        self.render(&self.path.join(command).with_extension("stderr"))
    }

    pub fn run_command(&self, command: &str, code: i32) {
        sheldon(&self.root.path())
            .arg(command)
            .assert()
            .code(code)
            .stdout(self.stdout(command))
            .stderr(self.stderr(command));
    }

    pub fn run(&self) -> io::Result<()> {
        // Lock the configuration
        self.run_command("lock", 0);

        // Check that the plugins.lock file is correct
        assert_eq!(
            fs::read_to_string(&self.lock_file).expect("failed to read lock file"),
            self.render(&self.path.join("plugins.lock"))
        );

        // Source the configuration
        self.run_command("source", 0);

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
        .path()
        .join("repositories/github.com/rossmacarthur/sheldon-test");
    let filename = directory.join("test.plugin.zsh");
    assert!(directory.is_dir());
    assert!(filename.is_file());
    assert_eq!(
        git_status(&directory),
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
        .path()
        .join("repositories/github.com/rossmacarthur/sheldon-test");
    let filename = directory.join("test.plugin.zsh");
    assert!(directory.is_dir());
    assert!(filename.is_file());
    assert_eq!(
        git_status(&directory),
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
        .path()
        .join("repositories/github.com/rossmacarthur/sheldon-test");
    let filename = directory.join("test.plugin.zsh");
    assert!(directory.is_dir());
    assert!(filename.is_file());
    assert_eq!(
        git_status(&directory),
        r#"On branch master
Your branch is up to date with 'origin/master'.

nothing to commit, working tree clean
"#,
    );

    Ok(())
}

#[test]
fn git_bad_url() -> io::Result<()> {
    let case = TestCase::new("git_bad_url")?;

    case.run_command("lock", 1);

    // Check that the plugins.lock file wasn't created
    assert!(!case.lock_file.exists());

    case.run_command("source", 0);

    // Check that the plugins.lock file wasn't created
    assert!(!case.lock_file.exists());

    Ok(())
}
