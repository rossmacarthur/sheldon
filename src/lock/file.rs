//! The raw lock file.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as ResultExt, Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::config::{InlinePlugin, Template};
use crate::context::Context;

/// A locked `Config`.
#[derive(Debug, Deserialize, Serialize)]
pub struct LockedConfig {
    /// The global context that was used to generated this `LockedConfig`.
    #[serde(flatten)]
    pub ctx: Context,
    /// Each locked plugin.
    pub plugins: Vec<LockedPlugin>,
    /// A map of name to template.
    ///
    /// Note: this field must come last in the struct for it to serialize
    /// properly.
    pub templates: IndexMap<String, Template>,
    /// Any errors that occurred while generating this `LockedConfig`.
    #[serde(skip)]
    pub errors: Vec<Error>,
}

/// A locked `Plugin`.
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LockedPlugin {
    External(LockedExternalPlugin),
    Inline(InlinePlugin),
}

/// A locked `ExternalPlugin`.
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LockedExternalPlugin {
    /// The name of this plugin.
    pub name: String,
    /// The directory that this plugin's source resides in.
    pub source_dir: PathBuf,
    /// The directory that this plugin resides in (inside the source directory).
    pub plugin_dir: Option<PathBuf>,
    /// The files to use in the plugin directory.
    pub files: Vec<PathBuf>,
    /// What templates to apply to each file.
    pub apply: Vec<String>,
}

impl LockedConfig {
    /// Write a `LockedConfig` config to the given path.
    pub fn to_path<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        fs::write(
            path,
            &toml::to_string(&self).context("failed to serialize locked config")?,
        )
        .with_context(s!("failed to write locked config to `{}`", path.display()))?;
        Ok(())
    }
}
