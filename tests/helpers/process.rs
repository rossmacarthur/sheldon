use std::env;
use std::ffi::OsStr;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use pretty_assertions::assert_eq;

use crate::helpers::TestDirs;

pub struct TestCommand {
    command: Command,
    expect_exit_code: Option<i32>,
    expect_stdout: Option<String>,
    expect_stderr: Option<String>,
}

impl TestCommand {
    pub fn new(dirs: &TestDirs) -> Self {
        // https://github.com/rust-lang/cargo/blob/2af662e22177a839763ac8fb70d245a680b15214/crates/cargo-test-support/src/lib.rs#L427-L441
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
            env!("TARGET").replace('-', "_").to_ascii_uppercase()
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
            .env_clear()
            .env("HOME", dirs.home.path())
            .env("SHELDON_CONFIG_DIR", &dirs.config)
            .env("SHELDON_DATA_DIR", &dirs.data)
            .args(&params)
            .arg("--non-interactive")
            .arg("--verbose");

        Self {
            command,
            expect_exit_code: None,
            expect_stdout: None,
            expect_stderr: None,
        }
    }

    pub fn expect_exit_code(mut self, exit_code: i32) -> Self {
        self.expect_exit_code = Some(exit_code);
        self
    }

    pub fn expect_stdout(mut self, stdout: String) -> Self {
        self.expect_stdout = Some(stdout);
        self
    }

    pub fn expect_stderr(mut self, stderr: String) -> Self {
        self.expect_stderr = Some(stderr);
        self
    }

    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.env(key, val);
        self
    }

    pub fn env_remove<K>(mut self, key: K) -> Self
    where
        K: AsRef<OsStr>,
    {
        self.command.env_remove(key);
        self
    }

    pub fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.envs(vars);
        self
    }

    pub fn arg<S>(mut self, arg: S) -> Self
    where
        S: AsRef<OsStr>,
    {
        self.command.arg(arg);
        self
    }

    /// Run the command and assert that the output is as expected.
    #[track_caller]
    pub fn run(mut self) -> io::Result<()> {
        let result = self.command.output()?;
        let result_exit_code = result.status.code().unwrap();
        let result_stdout = String::from_utf8_lossy(&result.stdout);
        let result_stderr = String::from_utf8_lossy(&result.stderr);
        if let Some(exit_code) = self.expect_exit_code {
            assert_eq!(
                result_exit_code, exit_code,
                "
exit code: {result_exit_code}
stdout: {result_stdout}
stderr: {result_stderr}
",
            );
        }
        if let Some(stdout) = self.expect_stdout {
            assert_eq!(
                result_stdout, stdout,
                "
exit code: {result_exit_code}
stdout: {result_stdout}
stderr: {result_stderr}
",
            );
        }
        if let Some(stderr) = self.expect_stderr {
            assert_eq!(
                result_stderr, stderr,
                "
exit code: {result_exit_code}
stdout: {result_stdout}
stderr: {result_stderr}
",
            );
        }
        Ok(())
    }
}
