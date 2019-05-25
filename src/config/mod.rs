//! Plugin configuration.

mod file;

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::Result;

use self::file::RawConfig;

/////////////////////////////////////////////////////////////////////////
// Configuration definitions
/////////////////////////////////////////////////////////////////////////

/// A wrapper around a template string.
#[derive(Debug, PartialEq, Serialize)]
pub struct Template {
    /// The actual template string.
    pub value: String,
    /// Whether this template should be applied to each filename.
    pub each: bool,
}

/// A Git reference.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GitReference {
    /// From the HEAD of a branch.
    Branch(String),
    /// From a specific revision.
    Revision(String),
    /// From a tag.
    Tag(String),
}

/// The source for a `Plugin`.
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
    Local { directory: PathBuf },
}

/// A configured plugin.
#[derive(Debug)]
pub struct Plugin {
    /// The name of this plugin.
    pub name: String,
    /// Specifies how to retrieve this plugin.
    pub source: Source,
    /// What files to use in the plugin's directory.
    pub uses: Option<Vec<String>>,
    /// What templates to apply to each matched file.
    pub apply: Option<Vec<String>>,
}

/// The user configuration.
#[derive(Debug)]
pub struct Config {
    /// Which files to match and use in a plugin's directory.
    pub matches: Vec<String>,
    /// The default list of template names to apply to each matched file.
    pub apply: Vec<String>,
    /// A map of name to template string.
    pub templates: IndexMap<String, Template>,
    /// Each configured plugin.
    pub plugins: Vec<Plugin>,
}

impl Template {
    /// Set whether this `Template` should be applied to every filename.
    pub fn each(mut self, each: bool) -> Self {
        self.each = each;
        self
    }
}

impl Config {
    /// Read a `Config` from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(RawConfig::from_path(path)?.normalize()?)
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn deserialize_config_from_path_example() {
        let mut path: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        path.push("docs/plugins.example.toml");
        Config::from_path(path).unwrap();
    }
}
