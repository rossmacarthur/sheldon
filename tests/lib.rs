mod helpers;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use pretty_assertions::assert_eq;

use crate::helpers::{TestCommand, TestDirs};

////////////////////////////////////////////////////////////////////////////////
// Utilities
////////////////////////////////////////////////////////////////////////////////

struct TestCase {
    dirs: TestDirs,
    data: HashMap<String, String>,
}

impl TestCase {
    /// Load the test case with the given name.
    fn load(name: &str) -> io::Result<Self> {
        let dirs = TestDirs::default()?;
        Self::load_with_dirs(name, dirs)
    }

    /// Load the test case in the given directories.
    fn load_with_dirs(name: &str, dirs: TestDirs) -> io::Result<Self> {
        static ENGINE: Lazy<upon::Engine> = Lazy::new(|| {
            let syntax = upon::Syntax::builder().expr("<", ">").build();
            upon::Engine::with_syntax(syntax)
        });

        let dir = {
            let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            p.push("tests/testdata");
            p.push(name);
            p
        };

        let subs = upon::value! {
            version: env!("CARGO_PKG_VERSION"),
            home: dirs.home.path(),
            config: &dirs.config,
            data: &dirs.data,
        };

        let mut data = HashMap::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            let raw = fs::read_to_string(&path)?;
            let value = ENGINE.compile(&raw).unwrap().render(&subs).unwrap();
            data.insert(name, value);
        }

        Ok(Self { dirs, data })
    }

    /// Get the value of the given key in this test case.
    fn get<S>(&self, key: S) -> String
    where
        S: AsRef<str>,
    {
        self.data.get(key.as_ref()).cloned().unwrap_or_default()
    }

    fn command(&self, command: &str) -> TestCommand {
        TestCommand::new(&self.dirs)
            .expect_exit_code(0)
            .expect_stdout(self.get(format!("{command}.stdout")))
            .expect_stderr(self.get(format!("{command}.stderr")))
            .arg(command)
    }

    fn write_config_file(&self, name: &str) -> io::Result<()> {
        fs::write(self.dirs.config.join(name), self.get(name))
    }

    fn write_file(&self, path: &Path, name: &str) -> io::Result<()> {
        fs::write(path, self.get(name))
    }

    fn assert_contents(&self, name: &str) -> io::Result<()> {
        self.assert_contents_path(name, &self.dirs.data.join(name))
    }

    fn assert_contents_path(&self, name: &str, path: &Path) -> io::Result<()> {
        assert_eq!(&fs::read_to_string(path)?, &self.get(name));
        Ok(())
    }

    fn run(&self) -> io::Result<()> {
        self.write_config_file("plugins.toml")?;
        self.command("lock").run()?;
        self.assert_contents("plugins.lock")?;
        self.command("source").run()?;
        Ok(())
    }
}

trait RepositoryExt {
    fn revparse_commit(&self, spec: &str) -> Result<git2::Commit<'_>, git2::Error>;
    fn status(&self) -> Result<git2::Statuses<'_>, git2::Error>;
}

impl RepositoryExt for git2::Repository {
    fn revparse_commit(&self, spec: &str) -> Result<git2::Commit<'_>, git2::Error> {
        self.revparse_single(spec)?.peel_to_commit()
    }

    fn status(&self) -> Result<git2::Statuses<'_>, git2::Error> {
        self.statuses(Some(git2::StatusOptions::new().include_untracked(true)))
    }
}

// Check that sheldon-test was in fact cloned.
fn check_sheldon_test(data: &Path) -> Result<(), git2::Error> {
    let dir = data.join("repos/github.com/rossmacarthur/sheldon-test");
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

////////////////////////////////////////////////////////////////////////////////
// Tests
////////////////////////////////////////////////////////////////////////////////

#[test]
fn lock_and_source_clean() -> io::Result<()> {
    let case = TestCase::load("clean")?;
    let data = &case.dirs.data;
    fs::create_dir_all(data.join("repos/test.com"))?;
    {
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(data.join("repos/test.com/test.txt"))?;
    }

    case.run()?;

    Ok(())
}

#[test]
fn lock_and_source_clean_permission_denied() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let case = TestCase::load("clean_permission_denied")?;
    let data = &case.dirs.data;
    fs::create_dir_all(data.join("repos/test.com"))?;
    {
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(data.join("repos/test.com/test.txt"))?;
    }
    fs::set_permissions(
        data.join("repos/test.com"),
        fs::Permissions::from_mode(0o000),
    )?;

    case.run()?;

    fs::set_permissions(
        data.join("repos/test.com"),
        fs::Permissions::from_mode(0o777),
    )?;

    Ok(())
}

#[test]
fn lock_and_source_empty() -> io::Result<()> {
    TestCase::load("empty")?.run()
}

#[test]
#[ignore]
fn lock_and_source_github_git() -> io::Result<()> {
    let case = TestCase::load("github_git")?;
    case.run()?;
    check_sheldon_test(&case.dirs.data).unwrap();
    Ok(())
}

#[test]
fn lock_and_source_github_https() -> io::Result<()> {
    let case = TestCase::load("github_https")?;
    case.run()?;
    check_sheldon_test(&case.dirs.data).unwrap();
    Ok(())
}

#[test]
fn lock_and_source_github_branch() -> io::Result<()> {
    let case = TestCase::load("github_branch")?;
    case.run()?;

    // Check that sheldon-test@feature was in fact cloned.
    let dir = case
        .dirs
        .data
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
        .dirs
        .data
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
    check_sheldon_test(&case.dirs.data).unwrap();
    Ok(())
}

#[test]
fn lock_and_source_github_bad_url() -> io::Result<()> {
    let case = TestCase::load("github_bad_url")?;
    case.write_config_file("plugins.toml")?;
    case.command("lock").expect_exit_code(2).run()?;
    assert!(!case.dirs.data.join("plugins.lock").exists());
    case.command("source").run()?;
    assert!(!case.dirs.data.join("plugins.lock").exists());
    Ok(())
}

#[test]
fn lock_and_source_github_bad_reinstall() -> io::Result<()> {
    // first setup up a correct situation
    let case = TestCase::load("github_tag")?;
    case.run()?;
    check_sheldon_test(&case.dirs.data).unwrap();

    // Now use a bad URL and try reinstall
    let case = TestCase::load_with_dirs("github_bad_reinstall", case.dirs)?;
    case.write_config_file("plugins.toml")?;
    case.command("lock")
        .expect_exit_code(2)
        .arg("--reinstall")
        .run()?;

    // check that the previously installed plugin and lock file is okay
    check_sheldon_test(&case.dirs.data).unwrap();
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
    let config_file = case.dirs.home.path().join("test.toml");
    fs::remove_dir(&case.dirs.config).ok();
    case.write_file(&config_file, "test.toml")?;
    case.command("lock")
        .env("SHELDON_CONFIG_FILE", &config_file)
        .run()?;
    case.assert_contents("plugins.lock")?;
    case.command("source")
        .env("SHELDON_CONFIG_FILE", &config_file)
        .run()?;
    Ok(())
}

#[test]
fn lock_and_source_override_config_file_missing() -> io::Result<()> {
    let case = TestCase::load("override_config_file_missing")?;
    let config_file = case.dirs.config.join("test.toml");
    case.command("lock")
        .expect_exit_code(2)
        .env("SHELDON_CONFIG_FILE", &config_file)
        .run()?;
    case.command("source")
        .expect_exit_code(2)
        .env("SHELDON_CONFIG_FILE", &config_file)
        .run()?;
    Ok(())
}

#[test]
fn lock_and_source_override_data_dir() -> io::Result<()> {
    let case = TestCase::load("override_data_dir")?;
    let data_dir = case.dirs.home.path().join("test");
    case.write_config_file("plugins.toml")?;
    case.command("lock")
        .env("SHELDON_DATA_DIR", &data_dir)
        .run()?;
    case.assert_contents_path("plugins.lock", &data_dir.join("plugins.lock"))?;
    case.command("source")
        .env("SHELDON_DATA_DIR", &data_dir)
        .run()?;
    Ok(())
}

#[test]
fn lock_and_source_profiles() -> io::Result<()> {
    let case = TestCase::load("profiles")?;
    case.write_config_file("plugins.toml")?;
    case.command("lock").env("SHELDON_PROFILE", "p1").run()?;
    case.assert_contents("plugins.p1.lock")?;
    case.command("source").env("SHELDON_PROFILE", "p1").run()?;
    check_sheldon_test(&case.dirs.data).unwrap();
    Ok(())
}

#[test]
fn directories_old() -> io::Result<()> {
    let case = TestCase::load("directories_old")?;
    let config_dir = case.dirs.home.path().join(".sheldon");
    fs::remove_dir(&case.dirs.data).ok();
    fs::remove_dir(&case.dirs.config).ok();
    fs::create_dir_all(&config_dir)?;
    case.write_file(&config_dir.join("plugins.toml"), "plugins.toml")?;
    case.command("lock")
        .env_remove("SHELDON_CONFIG_DIR")
        .env_remove("SHELDON_DATA_DIR")
        .run()?;
    case.assert_contents_path("plugins.lock", &config_dir.join("plugins.lock"))?;
    case.command("source")
        .env_remove("SHELDON_CONFIG_DIR")
        .env_remove("SHELDON_DATA_DIR")
        .run()?;
    Ok(())
}

#[test]
fn directories_default() -> io::Result<()> {
    let dirs = TestDirs::default()?;
    let case = TestCase::load_with_dirs("directories_default", dirs)?;
    case.write_config_file("plugins.toml")?;
    case.command("lock").run()?;
    case.assert_contents("plugins.lock")?;
    case.command("source").run()?;
    case.dirs.assert_conforms();
    Ok(())
}

#[test]
fn directories_xdg_from_env() -> io::Result<()> {
    let dirs = TestDirs::new("config_custom/sheldon", ".local/custom/sheldon")?;
    let case = TestCase::load_with_dirs("directories_xdg_from_env", dirs)?;
    let xdg_config = case.dirs.home.path().join("config_custom");
    let xdg_data = case.dirs.home.path().join(".local/custom");
    let envs = [
        ("XDG_CONFIG_HOME", &xdg_config),
        ("XDG_DATA_HOME", &xdg_data),
    ];
    case.write_config_file("plugins.toml")?;
    case.command("lock")
        .env_remove("SHELDON_CONFIG_DIR")
        .env_remove("SHELDON_DATA_DIR")
        .envs(envs)
        .run()?;
    case.assert_contents("plugins.lock")?;
    case.command("source")
        .env_remove("SHELDON_CONFIG_DIR")
        .env_remove("SHELDON_DATA_DIR")
        .envs(envs)
        .run()?;
    case.dirs.assert_conforms();
    Ok(())
}

#[test]
fn version() -> io::Result<()> {
    let dirs = TestDirs::default()?;

    let maybe_commit_hash = option_env!("GIT_COMMIT_SHORT_HASH");
    let maybe_commit_date = option_env!("GIT_COMMIT_DATE");
    let commit_info =
        if let (Some(commit_hash), Some(commit_date)) = (maybe_commit_hash, maybe_commit_date) {
            format!(" ({commit_hash} {commit_date})")
        } else {
            "".to_string()
        };

    let expected = format!(
        "{} {}{}\n{}\n",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        commit_info,
        env!("RUSTC_VERSION_SUMMARY")
    );
    TestCommand::new(&dirs)
        .arg("--version")
        .expect_exit_code(0)
        .expect_stdout(expected)
        .run()?;
    Ok(())
}
