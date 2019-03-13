mod error;
mod logging;

use std::{
    collections::HashMap,
    env, fmt, fs,
    path::{Path, PathBuf},
    result,
};

use clap::crate_name;
use indexmap::IndexMap;
use log::{debug, info, warn};
use maplit::hashmap;
use serde::{
    de::{self, Deserialize, MapAccess, Visitor},
    Deserializer,
};
use serde_derive::{Deserialize, Serialize};

pub use crate::{
    error::{Error, ErrorKind, Result},
    logging::init_logging,
};

/////////////////////////////////////////////////////////////////////////
// Utilities
/////////////////////////////////////////////////////////////////////////

/// A simple macro to call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

/// A simple macro to call .into() on each key and value in a hashmap!
/// initialization.
macro_rules! hashmap_into {
    ($($key:expr => $value:expr),*) => (hashmap!{$($key.into() => $value.into()),*})
}

/// Visitor to deserialize a [`Template`] as a string or a struct.
///
/// From https://stackoverflow.com/questions/54761790.
///
/// [`Template`]: struct.Template.html
struct TemplateVisitor;

impl<'de> Visitor<'de> for TemplateVisitor {
    type Value = Template;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string or map")
    }

    fn visit_str<E>(self, value: &str) -> result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(From::from(value))
    }

    fn visit_map<M>(self, visitor: M) -> result::Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let LockedTemplate { value, each } =
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(visitor))?;
        Ok(Template { value, each })
    }
}

/////////////////////////////////////////////////////////////////////////
// Configuration definitions
/////////////////////////////////////////////////////////////////////////

/// The default clone directory for repositories.
const CLONE_DIRECTORY: &str = "repositories";

/// The GitHub domain host.
const GITHUB_HOST: &str = "github.com";

/// The source type for a [`Plugin`].
///
/// [`Plugin`]: struct.Plugin.html
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase", tag = "source")]
pub enum Source {
    /// A clonable Git repository.
    Git {
        url: String,
        revision: Option<String>,
    },
    /// A GitHub repository, only the the username/repository needs to be
    /// specified.
    GitHub {
        repository: String,
        revision: Option<String>,
    },
    /// A local directory.
    Local { directory: PathBuf },
}

/// The source type for a [`NormalizedPlugin`].
///
/// [`NormalizedPlugin`]: struct.NormalizedPlugin.html
#[derive(Clone, Debug, PartialEq)]
enum NormalizedSource {
    /// A clonable Git repository.
    Git {
        url: String,
        revision: Option<String>,
    },
    /// A local directory.
    Local,
}

/// A configured shell plugin.
// Note: we would want to use #[serde(deny_unknown_fields)] here but it doesn't
// work with a flattened internally-tagged enum.
// See https://github.com/serde-rs/serde/issues/1358.
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Plugin {
    /// Specifies how to retrieve this plugin.
    #[serde(flatten)]
    source: Source,
    /// Which files to use in this plugin's directory. If this is `None` then
    /// this will figured out based on the global [`matches`] field.
    ///
    /// [`matches`]: struct.Config.html#structconfig.matches
    #[serde(rename = "use")]
    uses: Option<Vec<String>>,
    /// What templates to apply to each matched file. If this is `None` then the
    /// default templates will be applied.
    apply: Option<Vec<String>>,
}

/// A normalized [`Plugin`].
///
/// [`Plugin`]: struct.Plugin.html
#[derive(Clone, Debug, PartialEq)]
struct NormalizedPlugin {
    /// The name of this plugin.
    name: String,
    /// Specifies how to retrieve this plugin.
    source: NormalizedSource,
    /// The directory that this plugin resides in.
    directory: PathBuf,
    /// What files to use in the plugin's directory.
    uses: Option<Vec<String>>,
    /// What templates to apply to each matched file.
    apply: Vec<String>,
}

/// A locked [`Plugin`].
///
/// [`Plugin`]: struct.Plugin.html
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct LockedPlugin {
    /// The name of this plugin.
    name: String,
    /// The directory that this plugin resides in.
    directory: PathBuf,
    /// The filenames to use in the directory.
    filenames: Vec<PathBuf>,
    /// What templates to apply to each filename..
    apply: Vec<String>,
}

/// A wrapper around a template string.
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Template {
    /// The actual template string.
    value: String,
    /// Whether this template should be applied to each filename.
    each: bool,
}

/// A locked [`Template`].
///
/// This is exactly the same as a `Template` but we don't want to allow string
/// deserialization.
///
/// [`Template`]: struct.Template.html
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct LockedTemplate {
    /// The actual template string.
    value: String,
    /// Whether this template should be applied to each filename.
    each: bool,
}

/// The contents of a configuration file.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Config {
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    matches: Vec<String>,
    /// The default list of template names to apply to each matched file.
    apply: Vec<String>,
    /// A map of name to template string.
    templates: HashMap<String, Template>,
    /// Each configured plugin.
    plugins: IndexMap<String, Plugin>,
}

/// A locked [`Config`].
///
/// [`Config`]: struct.Config.html
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct LockedConfig {
    /// The root folder used.
    root: PathBuf,
    /// A map of name to template.
    templates: HashMap<String, LockedTemplate>,
    /// Each locked plugin.
    plugins: Vec<LockedPlugin>,
}

/////////////////////////////////////////////////////////////////////////
// Configuration implementations
/////////////////////////////////////////////////////////////////////////

impl Default for Config {
    /// Returns the default `Config`.
    fn default() -> Self {
        Config {
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
            plugins: IndexMap::new(),
        }
    }
}

impl Source {
    /// Return the directory for this [`Source`].
    ///
    /// For a local source this is simply the directory defined. For a Git
    /// or GitHub source this is the path to the repository's
    /// directory where the repository is cloned. It adheres to the following
    /// format:
    ///
    /// ```text
    ///   repositories
    ///   └── github.com
    ///       └── rossmacarthur
    ///           └── pure
    /// ```
    ///
    /// # Errors
    ///
    /// - If a Git URL cannot be parsed.
    /// - If a GitHub repository cannot be parsed into a username and
    ///   repository.
    ///
    /// [`Source`]: enum.Source.html
    pub fn directory(&self, root: &Path) -> Result<PathBuf> {
        let root = root.to_str().expect("root directory is not valid UTF-8");

        Ok(match self {
            Source::Git { url, .. } => {
                let error = || Error::config_git(url);

                // Parse the URL and generate a directory based on the URL.
                let parsed = url::Url::parse(url).map_err(|_| Error::config_git(url))?;
                let base = vec![root, CLONE_DIRECTORY, parsed.host_str().ok_or_else(error)?];
                let segments: Vec<_> = parsed.path_segments().ok_or_else(error)?.collect();
                base.iter().chain(segments.iter()).collect()
            },
            Source::GitHub { repository, .. } => {
                let error = || Error::config_github(repository);

                // Split the GitHub identifier into username and repository name.
                let mut repo_split = repository.splitn(2, '/');
                let user = repo_split.next().ok_or_else(error)?;
                let name = repo_split.next().ok_or_else(error)?;

                // Generate the name of the clone directory.
                [root, CLONE_DIRECTORY, GITHUB_HOST, user, name]
                    .iter()
                    .collect()
            },
            Source::Local { directory } => directory.clone(),
        })
    }

    /// Return the URL for this [`Source`].
    ///
    /// For a Git or GitHub source this is the URL to the remote repository.
    ///
    /// # Panics
    ///
    /// It is a programming error to call this method on a Local source, and
    /// this function will panic.
    ///
    /// [`Source`]: enum.Source.html
    fn url(&self) -> Option<String> {
        match self {
            Source::Git { url, .. } => Some(url.clone()),
            Source::GitHub { repository, .. } => {
                Some(format!("https://{}/{}", GITHUB_HOST, repository))
            },
            Source::Local { .. } => None,
        }
    }

    /// Consume the Source and convert it to a [`NormalizedSource`]
    ///
    /// [`NormalizedSource`]: struct.NormalizedSource.html
    fn normalize(self) -> NormalizedSource {
        let url = self.url();
        match self {
            Source::Git { revision, .. } | Source::GitHub { revision, .. } => {
                NormalizedSource::Git {
                    url: url.unwrap(),
                    revision,
                }
            },
            Source::Local { .. } => NormalizedSource::Local,
        }
    }
}

impl Plugin {
    /// Consume the Plugin and convert it to a [`NormalizedPlugin`].
    ///
    /// # Errors
    ///
    /// Any errors that can be returned by the [`Source::directory()`] method.
    ///
    /// [`NormalizedPlugin`]: struct.NormalizedPlugin.html
    /// [`Source::directory()`]: enum.Source.html#method.directory
    fn normalize(self, name: String, root: &Path, apply: &[String]) -> Result<NormalizedPlugin> {
        Ok(NormalizedPlugin {
            name,
            directory: self.source.directory(root)?,
            source: self.source.normalize(),
            uses: self.uses,
            apply: self.apply.unwrap_or_else(|| apply.to_vec()),
        })
    }
}

impl NormalizedPlugin {
    /// Consume the NormalizedPlugin and convert it to a [`LockedPlugin`].
    ///
    /// This main purpose of this method is to determine the exact filenames to
    /// use for a plugin.
    ///
    /// [`LockedPlugin`]: struct.LockedPlugin.html
    fn lock(self, root: &Path, matches: &[String]) -> Result<LockedPlugin> {
        // Determine all the filenames
        let mut filenames = Vec::new();

        // Data to use in template rendering
        let data = hashmap! {
            "root" => root.to_str().expect("root directory is not valid UTF-8"),
            "name" => &self.name,
            "directory" => self.directory.to_str().expect("plugin directory is not valid UTF-8"),
        };

        let mut templates = handlebars::Handlebars::new();
        templates.set_strict_mode(true);

        // If the plugin defined what files to use, we do all of them.
        if let Some(uses) = &self.uses {
            for u in uses {
                for p in glob::glob(
                    &self
                        .directory
                        .join(
                            templates
                                .render_template(u, &data)
                                .map_err(|e| Error::render(e, u))?,
                        )
                        .to_string_lossy(),
                )
                .unwrap()
                .filter_map(result::Result::ok)
                {
                    filenames.push(p)
                }
            }
        // Otherwise we try to figure it out ...
        } else {
            for g in matches {
                let mut matched = false;

                for p in glob::glob(
                    &self
                        .directory
                        .join(
                            templates
                                .render_template(g, &data)
                                .map_err(|e| Error::render(e, g))?,
                        )
                        .to_string_lossy(),
                )
                .unwrap()
                .filter_map(result::Result::ok)
                {
                    filenames.push(p);
                    matched = true;
                }

                if matched {
                    break;
                }
            }
        }

        Ok(LockedPlugin {
            name: self.name,
            directory: self.directory,
            filenames,
            apply: self.apply,
        })
    }
}

impl Template {
    /// Update whether this `Template` should be applied to all files or not.
    fn each(mut self, each: bool) -> Self {
        self.each = each;
        self
    }

    /// Consume the Template and convert it to a [`LockedTemplate`].
    ///
    /// [`LockedTemplate`]: struct.LockedTemplate.html
    fn lock(self) -> LockedTemplate {
        LockedTemplate {
            value: self.value,
            each: self.each,
        }
    }
}

/// Manually implement `Deserialize` for a [`Template`].
///
/// Unfortunately we can't use this https://serde.rs/string-or-struct.html, because
/// we are storing `Template`s in a map.
///
/// [`Template`]: struct.Template.html
impl<'de> Deserialize<'de> for Template {
    fn deserialize<D>(deserializer: D) -> result::Result<Template, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TemplateVisitor)
    }
}

impl From<String> for Template {
    fn from(s: String) -> Self {
        Template {
            value: s,
            each: false,
        }
    }
}

impl From<&str> for Template {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl From<&str> for LockedTemplate {
    fn from(s: &str) -> Self {
        LockedTemplate {
            value: s.to_string(),
            each: false,
        }
    }
}

impl Config {
    /// Read a Config from the given path.
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let manager = toml::from_str(&String::from_utf8_lossy(
            &fs::read(&path).map_err(|e| Error::deserialize(e, &path))?,
        ))
        .map_err(|e| Error::deserialize(e, &path))?;
        debug!("deserialized config from `{}`", path.to_string_lossy());
        Ok(manager)
    }

    /// Download all required dependencies for plugins.
    ///
    /// TODO: Download in parallel!
    fn download(plugins: &[NormalizedPlugin]) -> Result<()> {
        for plugin in plugins {
            if let NormalizedSource::Git { url, revision } = &plugin.source {
                // Clone or open the repository.
                let repo = match git2::Repository::clone(&url, &plugin.directory) {
                    Ok(repo) => {
                        info!("{} cloned (required for `{}`)", url, plugin.name);
                        repo
                    },
                    Err(e) => {
                        if e.code() != git2::ErrorCode::Exists {
                            return Err(Error::git_clone(e, &url));
                        } else {
                            info!("{} is already cloned (required for `{}`)", url, plugin.name);
                            git2::Repository::open(&plugin.directory).map_err(|e| {
                                Error::git_open(e, &plugin.directory)
                            })?
                        }
                    },
                };

                // Checkout the configured revision.
                if let Some(revision) = revision {
                    let error = |e| Error::git_checkout(e, revision);
                    let object = repo.revparse_single(revision).map_err(error)?;
                    repo.checkout_tree(&object, None).map_err(error)?;
                    info!(
                        "{} checked out at {} (required for `{}`)",
                        url, revision, plugin.name
                    );
                }
            }
        }
        Ok(())
    }

    /// Lock this Config.
    fn lock(self, root: &Path) -> Result<LockedConfig> {
        // Create a new map of normalized plugins
        let mut normalized_plugins = Vec::with_capacity(self.plugins.len());
        for (name, plugin) in self.plugins {
            normalized_plugins.push(plugin.normalize(name, root, &self.apply)?);
        }

        // Clone all repositories
        Self::download(&normalized_plugins)?;

        // Create a new map of locked plugins
        let mut locked_plugins = Vec::with_capacity(normalized_plugins.len());

        for plugin in normalized_plugins {
            locked_plugins.push(plugin.lock(root, &self.matches)?);
        }

        // Determine the templates.
        let mut templates = hashmap_into! {
            "PATH" => "export PATH=\"{{ directory }}:$PATH\"",
            "path" => "path=( \"{{ directory }}\" $path )",
            "fpath" => "fpath=( \"{{ directory }}\" $fpath )",
            "source" => Template::from("source \"{{ filename }}\"").each(true).lock()
        };

        // Add custom user templates.
        for (name, template) in self.templates {
            templates.insert(name, template.lock());
        }

        // Check that the templates can be compiled.
        {
            let mut templates_ = handlebars::Handlebars::new();
            templates_.set_strict_mode(true);
            for (name, template) in &templates {
                templates_
                    .register_template_string(&name, &template.value)
                    .map_err(|e| Error::template(e, name))?;
            }
        }

        Ok(LockedConfig {
            root: root.to_path_buf(),
            templates,
            plugins: locked_plugins,
        })
    }
}

impl LockedConfig {
    /// Read a LockedConfig from the given path.
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let locked = toml::from_str(&String::from_utf8_lossy(
            &fs::read(&path).map_err(|e| Error::deserialize(e, &path))?,
        ))
        .map_err(|e| Error::deserialize(e, &path))?;
        debug!(
            "deserialized locked config from `{}`",
            path.to_string_lossy()
        );
        Ok(locked)
    }

    /// Construct a new empty LockedConfig.
    fn new(root: &Path) -> Self {
        Config::default()
            .lock(root)
            .expect("failed to lock default Config")
    }

    /// Write a LockedConfig to the given path.
    fn to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        if let Some(directory) = path.parent() {
            if !directory.exists() {
                fs::create_dir_all(directory).map_err(|e| Error::serialize(e, &path))?;
                debug!("created directory `{}`", directory.to_string_lossy());
            }
        }

        fs::write(
            path,
            &toml::to_string_pretty(&self).map_err(|e| Error::serialize(e, &path))?,
        )
        .map_err(|e| Error::serialize(e, &path))?;
        debug!("wrote locked config to `{}`", path.to_string_lossy());
        Ok(())
    }

    /// Generate the shell script.
    fn source(&self) -> Result<String> {
        // Compile the templates
        let mut templates = handlebars::Handlebars::new();
        templates.set_strict_mode(true);
        for (name, template) in &self.templates {
            templates
                .register_template_string(&name, &template.value)
                .map_err(|e| Error::template(e, name))?;
        }

        let mut script = String::new();

        for plugin in &self.plugins {
            for name in &plugin.apply {
                // Data to use in template rendering
                let mut data = hashmap! {
                    "root" => self
                        .root
                        .to_str()
                        .expect("root directory is not valid UTF-8"),
                    "name" => &plugin.name,
                    "directory" => plugin
                        .directory
                        .to_str()
                        .expect("plugin directory is not valid UTF-8"),
                };

                if self.templates[name].each {
                    for filename in &plugin.filenames {
                        data.insert(
                            "filename",
                            filename.to_str().expect("filename is not valid UTF-8"),
                        );
                        script.push_str(
                            &templates
                                .render(name, &data)
                                .map_err(|e| Error::render(e, name))?,
                        );
                        script.push('\n');
                    }
                } else {
                    script.push_str(
                        &templates
                            .render(name, &data)
                            .map_err(|e| Error::render(e, name))?,
                    );
                    script.push('\n');
                }
            }
        }

        Ok(script)
    }
}

/////////////////////////////////////////////////////////////////////////
// Entry functions
/////////////////////////////////////////////////////////////////////////

/// General contextual information.
pub struct Context {
    root: PathBuf,
    config_file: PathBuf,
    lock_file: PathBuf,
}

impl Default for Context {
    fn default() -> Self {
        Self::defaults(None, None)
    }
}

impl Context {
    /// Determine the root directory and config file location.
    ///
    /// The root directory is determined using the following priority:
    /// - The given root value.
    /// - **Or** the environment variable `SHELDON_ROOT`.
    /// - **Or** the default root directory which is `$HOME/.zsh`.
    ///
    /// The config file path is determined using the following priority:
    /// - The given config file path.
    /// - **Or**`{{ root }}/plugins.toml`
    pub fn defaults(root: Option<&str>, config_file: Option<&str>) -> Self {
        let root = root.and_then(|s| Some(s.into())).unwrap_or_else(|| {
            env::var(format!("{}_ROOT", crate_name!().to_uppercase()))
                .and_then(|r| Ok(r.into()))
                .unwrap_or_else(|_| {
                    let mut root = dirs::home_dir().expect("failed to determine $HOME");
                    root.push(".zsh");
                    root
                })
        });
        debug!("using root directory `{}`", root.to_string_lossy());

        let config_file = config_file.and_then(|s| Some(s.into())).unwrap_or_else(|| {
            let mut config_file = root.clone();
            config_file.push("plugins.toml");
            config_file
        });
        debug!("using config file `{}`", config_file.to_string_lossy());

        let lock_file: PathBuf = format!(
            "{}{}",
            config_file.to_string_lossy().trim_end_matches(".toml"),
            ".lock"
        )
        .into();
        debug!("using lock file `{}`", lock_file.to_string_lossy());

        Context {
            root,
            config_file,
            lock_file,
        }
    }
}

/// Prepare a configuration for sourcing.
///
/// - Reads the config from the config file.
/// - Downloads all plugin dependencies.
/// - Generates a locked config.
/// - Writes the locked config to the lock file.
pub fn lock(ctx: &Context) -> Result<()> {
    Config::from_path(&ctx.config_file)?
        .lock(&ctx.root)?
        .to_path(&ctx.lock_file)?;
    Ok(())
}

/// Check if the config file is newer than the lock file.
fn config_file_newer(ctx: &Context) -> bool {
    let config_time = fs::metadata(&ctx.config_file)
        .ok()
        .and_then(|m| m.modified().ok());
    let lock_time = fs::metadata(&ctx.lock_file)
        .ok()
        .and_then(|m| m.modified().ok());

    lock_time.is_some() && config_time.is_some() && config_time.unwrap() > lock_time.unwrap()
}

/// Generate the init shell script.
///
/// - Reads the locked config from the lock file.
///     - If that fails the config will be read from the config path.
/// - Prints out the generated shell script.
pub fn source(ctx: &Context) -> Result<String> {
    let mut to_path = true;

    let locked = if config_file_newer(ctx) {
        info!("loaded new config");
        Config::from_path(&ctx.config_file)?.lock(&ctx.root)?
    } else {
        match LockedConfig::from_path(&ctx.lock_file) {
            Ok(locked) => {
                to_path = false;
                locked
            },
            Err(e) => {
                warn!("{}", e);
                match Config::from_path(&ctx.config_file) {
                    Ok(config) => config.lock(&ctx.root)?,
                    Err(e) => {
                        if e.source_is_io_not_found() {
                            warn!("{}", e);
                            LockedConfig::new(&ctx.root)
                        } else {
                            return Err(e);
                        }
                    },
                }
            },
        }
    };

    if to_path {
        locked.to_path(&ctx.lock_file)?;
    }

    locked.source()
}
