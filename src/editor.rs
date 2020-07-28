//! Open the config file in the default text editor.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use anyhow::{anyhow, bail, Context as ResultExt, Result};

use crate::edit;
use crate::util::TempPath;

/// Possible environment variables.
const ENV_VARS: &[&str] = &["VISUAL", "EDITOR"];

/// Possible editors to use.
#[cfg(not(target_os = "windows"))]
const EDITORS: &[&str] = &["code --wait", "nano", "vim", "vi", "emacs"];

/// Possible editors to use.
#[cfg(target_os = "windows")]
const EDITORS: &[&str] = &["code.exe --wait", "notepad.exe"];

/// Represents the default editor.
pub struct Editor {
    /// The path to the editor binary.
    bin: PathBuf,
    /// Extra args for the editor that might be required.
    args: Vec<String>,
}

/// Representation of a running or exited editor process.
pub struct Child {
    /// A handle for the editor child process.
    child: process::Child,
    /// The temporary file that the editor is editing.
    file: TempPath,
}

/// Convert a string command to a binary and the rest of the arguments.
fn to_bin_and_args<S>(cmd: S) -> Option<(PathBuf, Vec<String>)>
where
    S: AsRef<str>,
{
    let mut split = cmd.as_ref().split_whitespace();
    let bin: PathBuf = split.next()?.into();
    let args: Vec<String> = split.map(Into::into).collect();
    Some((bin, args))
}

impl Editor {
    /// Create a new default `Editor`.
    ///
    /// This function tries to read from `ENV_VARS` environment variables.
    /// Otherwise it will fallback to any of `EDITORS`.
    pub fn default() -> Result<Self> {
        let (bin, args) = ENV_VARS
            .iter()
            .filter_map(|e| env::var(e).ok())
            .filter_map(to_bin_and_args)
            .chain(EDITORS.iter().filter_map(to_bin_and_args))
            .find(|(bin, _)| which::which(bin).is_ok())
            .ok_or_else(|| anyhow!("failed to determine default editor"))?;
        Ok(Self { bin, args })
    }

    /// Open a file for editing with initial contents.
    pub fn edit(self, path: &Path, contents: &str) -> Result<Child> {
        let file = TempPath::new(path);
        let Self { bin, args } = self;
        fs::write(file.path(), &contents).context("failed to write to temporary file")?;
        let child = Command::new(bin)
            .args(args)
            .arg(file.path())
            .spawn()
            .context("failed to spawn editor subprocess")?;
        Ok(Child { child, file })
    }
}

impl Child {
    /// Wait for the child process to exit and then update the config file.
    pub fn wait_and_update(self, original_contents: &str) -> Result<edit::Config> {
        let Self { mut child, file } = self;
        let exit_status = child.wait()?;
        if exit_status.success() {
            let contents =
                fs::read_to_string(file.path()).context("failed to read from temporary file")?;
            if contents == original_contents {
                bail!("aborted editing!");
            } else {
                edit::Config::from_str(&contents)
                    .context("edited config is invalid, not updating config file")
            }
        } else {
            bail!("editor terminated with {}", exit_status)
        }
    }
}
