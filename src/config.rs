//! Defines the configuration file, and how to serialize and deserialize it.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use log::debug;
use serde_derive::{Deserialize, Serialize};

use crate::{Error, Result};

/// A simple macro to call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

/// The source type of a plugin.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase", tag = "source")]
pub enum Source {
    /// A clonable Git repository.
    Git { url: String },
    /// A repository on GitHub, only the the username/repository needs to be specified.
    GitHub { repository: String },
    /// A local directory.
    Local { directory: PathBuf },
    /// Hints that destructuring should not be exhaustive.
    // Until https://github.com/rust-lang/rust/issues/44109 is stabilized.
    #[doc(hidden)]
    __Nonexhaustive,
}

/// A configured plugin.
///
/// Defines how to retrieve and use this plugin.
///
// Note: we would want to use #[serde(deny_unknown_fields)] here but it doesn't work with a
// flattened internally-tagged enum. See https://github.com/serde-rs/serde/issues/1358.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Plugin {
    /// How to retrieve this plugin.
    #[serde(flatten)]
    source: Source,
    /// Which files to use in this plugin's directory. If this is `None` then this will figured out
    /// based on the global [`matches`] field.
    ///
    /// [`matches`]: struct.Global.html#structglobal.matches
    #[serde(rename = "use")]
    uses: Option<Vec<String>>,
    /// What templates to apply to each matched file. If this is `None` then the default templates
    /// will be applied.
    apply: Option<Vec<String>>,
}

/// Global settings that apply to all plugins.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Global {
    /// The root folder.
    root: Option<PathBuf>,
    /// Which files to match and use in a plugin's directory.
    ///
    /// This should be a list of glob patterns. This is slightly different to a plugin's [`uses`]
    /// field, in that this one only uses the first glob that returns more than zero files.
    ///
    /// [`uses`]: struct.Plugin.html#structfield.uses
    #[serde(rename = "match")]
    matches: Vec<String>,
    /// The default list of template names to apply to each matched file.
    apply: Vec<String>,
    /// A map of name to template of user configured templates.
    templates: HashMap<String, String>,
}

/// The contents of a configuration file.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The global settings.
    #[serde(flatten)]
    global: Global,
    /// Each configured plugin.
    // Note: this needs to be an `IndexMap` because the order in which the plugins are defined is
    // important.
    plugins: IndexMap<String, Plugin>,
}

impl Default for Global {
    /// Returns the default `Global`.
    fn default() -> Self {
        Global {
            root: None,
            templates: HashMap::new(),
            matches: vec_into![
                "{{ name }}.plugin.zsh",
                "{{ name }}.zsh",
                "{{ name }}.sh",
                "{{ name }}.zsh-theme",
                "*.plugin.zsh",
                "*.zsh",
                "*.sh",
                "*.zsh-theme"
            ],
            apply: vec_into!["source"],
        }
    }
}

impl Default for Config {
    /// Returns the default `Config`.
    fn default() -> Self {
        Config {
            global: Global::default(),
            plugins: IndexMap::new(),
        }
    }
}

impl Plugin {
    /// Construct a new `Plugin`.
    pub fn new(source: Source) -> Self {
        Plugin {
            source,
            uses: None,
            apply: None,
        }
    }

    /// Construct a new Git `Plugin`.
    pub fn new_git<S: Into<String>>(url: S) -> Self {
        Self::new(Source::Git { url: url.into() })
    }

    /// Construct a new GitHub `Plugin`.
    pub fn new_github<S: Into<String>>(repository: S) -> Self {
        Self::new(Source::GitHub {
            repository: repository.into(),
        })
    }

    /// Construct a new Local `Plugin`.
    pub fn new_local<P: Into<PathBuf>>(directory: P) -> Self {
        Self::new(Source::Local {
            directory: directory.into(),
        })
    }

    /// Set uses on this `Plugin`.
    pub fn uses(mut self, uses: Vec<String>) -> Self {
        self.uses = Some(uses);
        self
    }

    /// Set apply on this `Plugin`.
    pub fn apply(mut self, apply: Vec<String>) -> Self {
        self.apply = Some(apply);
        self
    }
}

impl Global {
    /// Construct a new empty `Global`.
    pub fn new() -> Self {
        Global::default()
    }

    /// Set the root directory.
    pub fn root<P: Into<PathBuf>>(mut self, root: P) -> Self {
        self.root = Some(root.into());
        self
    }

    /// Set the default matches.
    pub fn matches(mut self, matches: Vec<String>) -> Self {
        self.matches = matches;
        self
    }

    /// Set the default templates to apply.
    pub fn apply(mut self, apply: Vec<String>) -> Self {
        self.apply = apply;
        self
    }

    /// Add a template.
    pub fn template<S: Into<String>, T: Into<String>>(mut self, name: S, template: T) -> Self {
        self.templates.insert(name.into(), template.into());
        self
    }
}

impl Config {
    /// Construct a new empty `Config`.
    pub fn new() -> Self {
        Config {
            global: Global::default(),
            plugins: IndexMap::new(),
        }
    }

    /// Update the `Global` settings for this `Config`.
    pub fn global(mut self, global: Global) -> Self {
        self.global = global;
        self
    }

    /// Read a `Config` from the given path.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let manager = toml::from_str(&String::from_utf8_lossy(
            &fs::read(&path).map_err(|e| Error::deserialize(e, &path))?,
        ))
        .map_err(|e| Error::deserialize(e, &path))?;
        debug!("deserialized config from `{}`", path.to_string_lossy());
        Ok(manager)
    }

    /// Add a plugin to this `Config`.
    pub fn plugin<S: Into<String>>(mut self, name: S, plugin: Plugin) -> Self {
        self.plugins.insert(name.into(), plugin);
        self
    }
}
