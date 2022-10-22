//! The user configuration.

mod clean;
mod edit;
mod file;
mod normalize;
mod profile;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use anyhow::{Context as ResultExt, Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use url::Url;

pub use crate::config::clean::clean;
pub use crate::config::edit::{EditConfig, EditPlugin};
pub use crate::config::file::{GistRepository, GitHubRepository, GitProtocol, RawPlugin};
pub use crate::config::profile::MatchesProfile;

/// The user configuration.
#[derive(Debug)]
pub struct Config {
    /// What type of shell is being used.
    pub shell: Shell,
    /// Which files to match and use in a plugin's directory.
    pub matches: Option<Vec<String>>,
    /// The default list of template names to apply to each matched file.
    pub apply: Option<Vec<String>>,
    /// A map of name to template string.
    pub templates: IndexMap<String, String>,
    /// Each configured plugin.
    pub plugins: Vec<Plugin>,
}

/// The type of shell that we are using.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shell {
    Bash,
    Zsh,
}

/// A configured plugin.
#[derive(Debug, PartialEq, Eq)]
pub enum Plugin {
    External(ExternalPlugin),
    Inline(InlinePlugin),
}

/// An external configured plugin.
#[derive(Debug, PartialEq, Eq)]
pub struct ExternalPlugin {
    /// The name of this plugin.
    pub name: String,
    /// Specifies how to retrieve this plugin.
    pub source: Source,
    /// Which directory to use in this plugin.
    pub dir: Option<String>,
    /// What files to use in the plugin's directory.
    pub uses: Option<Vec<String>>,
    /// What templates to apply to each matched file.
    pub apply: Option<Vec<String>>,
    /// Only use this plugin under one of the given profiles.
    pub profiles: Option<Vec<String>>,
    /// Hooks executed during template evaluation.
    pub hooks: Option<BTreeMap<String, String>>,
}

/// The source for a [`Plugin`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Source {
    /// A clonable Git repository.
    Git {
        url: Url,
        reference: Option<GitReference>,
    },
    /// A remote file.
    Remote { url: Url },
    /// A local directory.
    Local { dir: PathBuf },
}

/// A Git reference.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitReference {
    /// From the tip of a branch.
    Branch(String),
    /// From a specific revision.
    Rev(String),
    /// From a tag.
    Tag(String),
}

/// An inline configured plugin.
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct InlinePlugin {
    /// The name of this plugin.
    pub name: String,
    /// The actual source.
    pub raw: String,
    /// Only use this plugin under one of the given profiles.
    pub profiles: Option<Vec<String>>,
    /// Hooks executed during template evaluation.
    pub hooks: Option<BTreeMap<String, String>>,
}

/// Load a [`Config`] from the given path.
pub fn from_path<P>(path: P, warnings: &mut Vec<Error>) -> Result<Config>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let bytes =
        fs::read(path).with_context(|| format!("failed to read from `{}`", path.display()))?;
    let contents = String::from_utf8(bytes).context("config file contents are not valid UTF-8")?;
    let raw_config = toml::from_str(&contents).context("failed to deserialize contents as TOML")?;
    normalize::normalize(raw_config, warnings)
}
