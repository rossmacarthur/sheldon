use std::collections::HashMap;
use std::env;
use std::ffi;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use itertools::Itertools;
use pest::Parser;
use pest_derive::Parser;
use pretty_assertions::assert_eq;

/////////////////////////////////////////////////////////////////////////
// Utilities
/////////////////////////////////////////////////////////////////////////

#[derive(Parser)]
#[grammar = "../tests/case.pest"]
struct TestCaseParser;

struct TestCommand {
    command: Command,
    expect_exit_code: Option<i32>,
    expect_stdout: Option<String>,
    expect_stderr: Option<String>,
}

struct TestCase {
    root: tempfile::TempDir,
    data: HashMap<String, String>,
}

impl TestCommand {
    fn new<R>(root: R) -> Self
    where
        R: AsRef<Path>,
    {
        // From: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/lib.rs#L545-L558
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

        let mut command = Command::new(&bin);
        let mut params = Vec::new();

        if let Ok(runner) = env::var(format!(
            "CARGO_TARGET_{}_RUNNER",
            env!("TARGET").replace("-", "_").to_ascii_uppercase()
        )) {
            let mut split = runner.splitn(2, char::is_whitespace);
            let runner_bin = split.next().unwrap();
            if let Some(runner_args) = split.next() {
                params = runner_args.split_whitespace().map(String::from).collect();
            }
            params.push(bin.as_path().to_string_lossy().to_string());
            command = Command::new(runner_bin);
        }

        command
            .env("HOME", root.as_ref())
            .env("SHELDON_ROOT", root.as_ref())
            .env_remove("SHELDON_CONFIG_FILE")
            .env_remove("SHELDON_LOCK_FILE")
            .env_remove("SHELDON_CLONE_DIR")
            .env_remove("SHELDON_DOWNLOAD_DIR")
            .args(&params)
            .arg("--verbose")
            .arg("--no-color");

        Self {
            command,
            expect_exit_code: None,
            expect_stdout: None,
            expect_stderr: None,
        }
    }

    fn expect_exit_code(mut self, exit_code: i32) -> Self {
        self.expect_exit_code = Some(exit_code);
        self
    }

    fn expect_stdout(mut self, stdout: String) -> Self {
        self.expect_stdout = Some(stdout);
        self
    }

    fn expect_stderr(mut self, stderr: String) -> Self {
        self.expect_stderr = Some(stderr);
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<ffi::OsStr>,
    {
        self.command.args(args);
        self
    }

    fn arg<S>(mut self, arg: S) -> Self
    where
        S: AsRef<ffi::OsStr>,
    {
        self.command.arg(arg);
        self
    }

    fn run(mut self) -> io::Result<()> {
        let result = self.command.output()?;
        if let Some(exit_code) = self.expect_exit_code {
            assert_eq!(result.status.code().unwrap(), exit_code);
        }
        if let Some(stdout) = self.expect_stdout {
            assert_eq!(String::from_utf8_lossy(&result.stdout), stdout);
        }
        if let Some(stderr) = self.expect_stderr {
            assert_eq!(String::from_utf8_lossy(&result.stderr), stderr);
        }
        Ok(())
    }
}

impl TestCase {
    /// Load the test case with the given name.
    fn load(name: &str) -> io::Result<Self> {
        let root = tempfile::tempdir()?;
        Self::load_with_root(name, root)
    }

    /// Load the test case in the given root directory.
    fn load_with_root(name: &str, root: tempfile::TempDir) -> io::Result<Self> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases")
            .join(name);
        let case = &fs::read_to_string(path)?;
        let parsed = TestCaseParser::parse(Rule::case, &case).expect("failed to parse case");
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
        Ok(Self { root, data })
    }

    /// Get the value of the given key in this test case.
    fn get<S>(&self, key: S) -> String
    where
        S: fmt::Display,
    {
        self.data
            .get(&key.to_string())
            .unwrap_or_else(|| panic!("expected `{}` to be present", key))
            .clone()
    }

    fn run_command(&self, command: &str) -> io::Result<()> {
        TestCommand::new(self.root.path())
            .expect_exit_code(0)
            .expect_stdout(self.get(format!("{}.stdout", command)))
            .expect_stderr(self.get(format!("{}.stderr", command)))
            .arg(command)
            .run()
    }

    fn write_file(&self, name: &str) -> io::Result<()> {
        fs::write(&self.root.path().join(name), self.get(name))
    }

    fn assert_contents(&self, name: &str) -> io::Result<()> {
        assert_eq!(
            &fs::read_to_string(&self.root.path().join(name))?,
            &self.get(name)
        );
        Ok(())
    }

    fn run(&self) -> io::Result<()> {
        self.write_file("plugins.toml")?;
        self.run_command("lock")?;
        self.assert_contents("plugins.lock")?;
        self.run_command("source")
    }
}

trait RepositoryExt {
    fn revparse_commit(&self, spec: &str) -> Result<git2::Commit, git2::Error>;
    fn status(&self) -> Result<git2::Statuses, git2::Error>;
}

impl RepositoryExt for git2::Repository {
    fn revparse_commit(&self, spec: &str) -> Result<git2::Commit, git2::Error> {
        Ok(self.revparse_single(spec)?.peel_to_commit()?)
    }

    fn status(&self) -> Result<git2::Statuses, git2::Error> {
        self.statuses(Some(git2::StatusOptions::new().include_untracked(true)))
    }
}

// Check that sheldon-test was in fact cloned.
fn check_sheldon_test(root: &Path) -> Result<(), git2::Error> {
    let dir = root.join("repos/github.com/rossmacarthur/sheldon-test");
    let file = dir.join("test.plugin.zsh");
    assert!(dir.is_dir());
    assert!(file.is_file());
    let repo = git2::Repository::open(&dir)?;
    // HEAD is the same as origin/master
    assert_eq!(
        repo.revparse_commit("HEAD")?.id(),
        repo.revparse_commit("origin/master")?.id()
    );
    // working tree clean
    assert!(repo.status()?.is_empty());
    Ok(())
}

/////////////////////////////////////////////////////////////////////////
// Tests
/////////////////////////////////////////////////////////////////////////

#[test]
fn lock_and_source_clean() -> io::Result<()> {
    let case = TestCase::load("clean")?;
    let root = case.root.path();
    fs::create_dir_all(&root.join("repos/test.com"))?;
    {
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&root.join("repos/test.com/test.txt"))?;
    }

    case.run()?;

    Ok(())
}

#[test]
fn lock_and_source_clean_permission_denied() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let case = TestCase::load("clean_permission_denied")?;
    let root = case.root.path();
    fs::create_dir_all(&root.join("repos/test.com"))?;
    {
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&root.join("repos/test.com/test.txt"))?;
    }
    fs::set_permissions(
        &root.join("repos/test.com"),
        fs::Permissions::from_mode(0o000),
    )?;

    case.run()?;

    fs::set_permissions(
        &root.join("repos/test.com"),
        fs::Permissions::from_mode(0o777),
    )?;

    Ok(())
}

#[test]
fn lock_and_source_empty() -> io::Result<()> {
    TestCase::load("empty")?.run()
}

#[test]
fn lock_and_source_github_git() -> io::Result<()> {
    let case = TestCase::load("github_git")?;
    case.run()?;
    check_sheldon_test(case.root.path()).unwrap();
    Ok(())
}

#[test]
fn lock_and_source_github_https() -> io::Result<()> {
    let case = TestCase::load("github_https")?;
    case.run()?;
    check_sheldon_test(case.root.path()).unwrap();
    Ok(())
}

#[test]
fn lock_and_source_github_branch() -> io::Result<()> {
    let case = TestCase::load("github_branch")?;
    case.run()?;

    // Check that sheldon-test@feature was in fact cloned.
    let dir = case
        .root
        .path()
        .join("repos/github.com/rossmacarthur/sheldon-test");
    let file = dir.join("test.plugin.zsh");
    assert!(dir.is_dir());
    assert!(file.is_file());

    let repo = git2::Repository::open(&dir).unwrap();
    // HEAD is 1 commit ahead of origin/master
    assert_eq!(
        repo.revparse_commit("HEAD~1").unwrap().id(),
        repo.revparse_commit("origin/master").unwrap().id()
    );
    // working tree clean
    assert!(repo.status().unwrap().is_empty());

    Ok(())
}

#[test]
fn lock_and_source_github_submodule() -> io::Result<()> {
    let case = TestCase::load("github_submodule")?;
    case.run()?;

    // Check that sheldon-test@recursive-recursive was in fact cloned.
    let dir = case
        .root
        .path()
        .join("repos/github.com/rossmacarthur/sheldon-test");
    let file = dir.join("test.plugin.zsh");
    assert!(dir.is_dir());
    assert!(file.is_file());
    let repo = git2::Repository::open(&dir).unwrap();
    // HEAD is 2 commits head of origin/master
    assert_eq!(
        repo.revparse_commit("HEAD~2").unwrap().id(),
        repo.revparse_commit("origin/master").unwrap().id()
    );
    // working tree clean
    assert!(repo.status().unwrap().is_empty());

    // Check that sheldon-test@recursive submodule self was in fact cloned.
    let dir = dir.join("self");
    let file = dir.join("test.plugin.zsh");
    assert!(dir.is_dir());
    assert!(file.is_file());
    let repo = git2::Repository::open(&dir).unwrap();
    // HEAD is 1 commits head of origin/master
    assert_eq!(
        repo.revparse_commit("HEAD~1").unwrap().id(),
        repo.revparse_commit("origin/master").unwrap().id()
    );
    // working tree clean
    assert!(repo.status().unwrap().is_empty());

    // Check that sheldon-test submodule was in fact cloned.
    let dir = dir.join("self");
    let file = dir.join("test.plugin.zsh");
    assert!(dir.is_dir());
    assert!(file.is_file());
    let repo = git2::Repository::open(&dir).unwrap();
    // HEAD is origin/master
    assert_eq!(
        repo.revparse_commit("HEAD").unwrap().id(),
        repo.revparse_commit("origin/master").unwrap().id()
    );
    // working tree clean
    assert!(repo.status().unwrap().is_empty());

    Ok(())
}

#[test]
fn lock_and_source_github_tag() -> io::Result<()> {
    let case = TestCase::load("github_tag")?;
    case.run()?;
    check_sheldon_test(case.root.path()).unwrap();
    Ok(())
}

#[test]
fn lock_and_source_github_bad_url() -> io::Result<()> {
    let case = TestCase::load("github_bad_url")?;
    case.write_file("plugins.toml")?;

    TestCommand::new(case.root.path())
        .expect_exit_code(2)
        .expect_stdout(case.get("lock.stdout"))
        .expect_stderr(case.get("lock.stderr"))
        .arg("lock")
        .run()?;

    assert!(!case.root.path().join("plugins.lock").exists());

    case.run_command("source")?;

    assert!(!case.root.path().join("plugins.lock").exists());

    Ok(())
}

#[test]
fn lock_and_source_github_bad_reinstall() -> io::Result<()> {
    // first setup up a correct situation
    let case = TestCase::load("github_tag")?;
    case.run()?;
    check_sheldon_test(case.root.path()).unwrap();

    // Now use a bad URL and try reinstall
    let case = TestCase::load_with_root("github_bad_reinstall", case.root)?;
    case.write_file("plugins.toml")?;
    TestCommand::new(case.root.path())
        .expect_exit_code(2)
        .expect_stdout(case.get("lock.stdout"))
        .expect_stderr(case.get("lock.stderr"))
        .arg("lock")
        .arg("--reinstall")
        .run()?;

    // check that the previously installed plugin and lock file is okay
    check_sheldon_test(case.root.path()).unwrap();
    case.assert_contents("plugins.lock")?;

    Ok(())
}

#[test]
fn lock_and_source_inline() -> io::Result<()> {
    TestCase::load("inline")?.run()
}

#[test]
fn lock_and_source_override_config_file() -> io::Result<()> {
    let case = TestCase::load("override_config_file")?;
    let config_file = case.root.path().join("test.toml");
    let args = ["--config-file", config_file.to_str().unwrap()];

    case.write_file("test.toml")?;

    TestCommand::new(case.root.path())
        .expect_exit_code(0)
        .expect_stdout(case.get("lock.stdout"))
        .expect_stderr(case.get("lock.stderr"))
        .args(&args)
        .arg("lock")
        .run()?;

    TestCommand::new(case.root.path())
        .expect_exit_code(0)
        .expect_stdout(case.get("source.stdout"))
        .expect_stderr(case.get("source.stderr"))
        .args(&args)
        .arg("source")
        .run()?;

    case.assert_contents("plugins.lock")
}

#[test]
fn lock_and_source_override_config_file_missing() -> io::Result<()> {
    let case = TestCase::load("override_config_file_missing")?;
    let config_file = case.root.path().join("test.toml");
    let args = ["--config-file", config_file.to_str().unwrap()];

    TestCommand::new(case.root.path())
        .expect_exit_code(2)
        .expect_stdout(case.get("stdout"))
        .expect_stderr(case.get("stderr"))
        .args(&args)
        .arg("lock")
        .run()?;

    TestCommand::new(case.root.path())
        .expect_exit_code(2)
        .expect_stdout(case.get("stdout"))
        .expect_stderr(case.get("stderr"))
        .args(&args)
        .arg("source")
        .run()
}

#[test]
fn lock_and_source_override_lock_file() -> io::Result<()> {
    let case = TestCase::load("override_lock_file")?;
    let lock_file = case.root.path().join("test.lock");
    let args = ["--lock-file", lock_file.to_str().unwrap()];

    case.write_file("plugins.toml")?;

    TestCommand::new(case.root.path())
        .expect_exit_code(0)
        .expect_stdout(case.get("lock.stdout"))
        .expect_stderr(case.get("lock.stderr"))
        .args(&args)
        .arg("lock")
        .run()?;

    case.assert_contents("test.lock")?;

    TestCommand::new(case.root.path())
        .expect_exit_code(0)
        .expect_stdout(case.get("source.stdout"))
        .expect_stderr(case.get("source.stderr"))
        .args(&args)
        .arg("source")
        .run()
}
