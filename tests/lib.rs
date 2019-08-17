use std::{
    collections::HashMap,
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use itertools::Itertools;
use pest::Parser;
use pest_derive::Parser;

/////////////////////////////////////////////////////////////////////////
// Utilities
/////////////////////////////////////////////////////////////////////////

fn sheldon<R>(root: R) -> Command
where
    R: AsRef<Path>,
{
    // From: https://github.com/rust-lang/cargo/blob/master/tests/testsuite/support/mod.rs#L542
    let bin = env::var_os("CARGO_BIN_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            env::current_exe().ok().map(|mut path| {
                path.pop();
                if path.ends_with("deps") {
                    path.pop();
                }
                path
            })
        })
        .unwrap()
        .join("sheldon");

    let mut command = Command::new(bin);
    command
        .env("HOME", root.as_ref())
        .env("SHELDON_ROOT", root.as_ref())
        .env_remove("SHELDON_CONFIG_FILE")
        .env_remove("SHELDON_LOCK_FILE")
        .arg("--verbose")
        .arg("--no-color");

    command
}

#[derive(Parser)]
#[grammar = "../tests/case.pest"]
struct TestCaseParser;

struct TestCase {
    root: tempfile::TempDir,
    config_path: PathBuf,
    lock_path: PathBuf,
    data: HashMap<String, String>,
}

impl TestCase {
    fn load(name: &str) -> io::Result<Self> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases")
            .join(name);
        let case = &fs::read_to_string(path)?;
        let parsed = TestCaseParser::parse(Rule::case, &case).expect("failed to parse case");
        let root = tempfile::tempdir()?;
        let config_path = root.path().join("plugins.toml");
        let lock_path = root.path().join("plugins.lock");
        let data: HashMap<String, String> = parsed
            .filter_map(|pair| {
                if pair.as_rule() == Rule::element {
                    pair.into_inner()
                        .map(|p| p.as_str().to_string())
                        .collect_tuple()
                        .map(|(k, v)| {
                            (
                                k,
                                v.replace("<root>", root.path().to_str().unwrap())
                                    .replace("<version>", env!("CARGO_PKG_VERSION")),
                            )
                        })
                } else {
                    None
                }
            })
            .collect();
        Ok(Self {
            root,
            config_path,
            lock_path,
            data,
        })
    }

    fn get(&self, key: &str) -> String {
        self.data[&key.to_string()].clone()
    }

    pub fn run_command(&self, command: &str, code: i32) -> io::Result<()> {
        let stdout = self.get(format!("{}.stdout", command).as_str());
        let stderr = self.get(format!("{}.stderr", command).as_str());
        let result = sheldon(&self.root.path()).arg(command).output()?;
        assert_eq!(result.status.code().unwrap(), code);
        assert_eq!(String::from_utf8_lossy(&result.stdout), stdout);
        assert_eq!(String::from_utf8_lossy(&result.stderr), stderr);
        Ok(())
    }

    pub fn run(&self) -> io::Result<()> {
        fs::write(&self.config_path, self.get("plugins.toml"))?;

        self.run_command("lock", 0)?;

        assert_eq!(
            &fs::read_to_string(&self.lock_path)?,
            &self.get("plugins.lock")
        );

        self.run_command("source", 0)?;

        Ok(())
    }
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

/////////////////////////////////////////////////////////////////////////
// Tests
/////////////////////////////////////////////////////////////////////////

#[test]
fn empty() -> io::Result<()> {
    TestCase::load("empty")?.run()
}

#[test]
fn git() -> io::Result<()> {
    let case = TestCase::load("git")?;
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
    let case = TestCase::load("git_branch")?;
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
    let case = TestCase::load("git_tag")?;
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
    let case = TestCase::load("git_bad_url")?;
    fs::write(&case.config_path, case.get("plugins.toml"))?;

    case.run_command("lock", 1)?;
    assert!(!case.lock_path.exists());

    case.run_command("source", 0)?;
    assert!(!case.lock_path.exists());

    Ok(())
}
