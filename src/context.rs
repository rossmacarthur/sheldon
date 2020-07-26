//! Contextual information.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{config::Shell, log::Output, util::PathExt};

/// Settings to use over the entire program.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Settings {
    /// The current crate version.
    pub version: String,
    /// The location of the home directory.
    pub home: PathBuf,
    /// The location of the root directory.
    pub root: PathBuf,
    /// The location of the config file.
    pub config_file: PathBuf,
    /// The location of the lock file.
    pub lock_file: PathBuf,
    /// The directory to clone git sources to.
    pub clone_dir: PathBuf,
    /// The directory to download remote plugins sources to.
    pub download_dir: PathBuf,
}

/// Contextual information to use across the entire program.
#[derive(Debug)]
pub struct Context<'a> {
    /// Common data.
    pub settings: &'a Settings,
    /// The output style.
    pub output: &'a Output,
}

/// Contextual information to use while running edit tasks (init, add, remove).
#[derive(Debug)]
pub struct EditContext {
    /// Common data.
    pub settings: Settings,
    /// The output style.
    pub output: Output,
    /// The type of shell.
    pub shell: Option<Shell>,
}

/// Contextual information to use while running the main tasks (lock and
/// source).
#[derive(Debug)]
pub struct LockContext {
    /// Common data.
    pub settings: Settings,
    /// The output style.
    pub output: Output,
    /// Whether to reinstall plugin sources.
    pub reinstall: bool,
}

macro_rules! setting_access {
    ($name:ident) => {
        #[inline]
        fn $name(&self) -> &Path {
            self.settings().$name.as_path()
        }
    };
}

/// Provides an interface to access `Settings` attributes.
pub trait SettingsExt {
    fn settings(&self) -> &Settings;

    setting_access!(home);

    setting_access!(root);

    setting_access!(config_file);

    setting_access!(lock_file);

    setting_access!(clone_dir);

    setting_access!(download_dir);

    /// Expands the tilde in the given path to the configured user's home
    /// directory.
    #[inline]
    fn expand_tilde(&self, path: PathBuf) -> PathBuf {
        path.expand_tilde(self.home())
    }

    /// Replaces the home directory in the given path with a tilde.
    #[inline]
    fn replace_home<P>(&self, path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        path.as_ref().replace_home(self.home())
    }
}

impl SettingsExt for Settings {
    #[inline]
    fn settings(&self) -> &Settings {
        self
    }
}

impl SettingsExt for Context<'_> {
    #[inline]
    fn settings(&self) -> &Settings {
        self.settings
    }
}

impl SettingsExt for EditContext {
    #[inline]
    fn settings(&self) -> &Settings {
        &self.settings
    }
}

impl SettingsExt for LockContext {
    #[inline]
    fn settings(&self) -> &Settings {
        &self.settings
    }
}
