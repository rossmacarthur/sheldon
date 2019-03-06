mod error;
mod logging;

use std::{
    borrow::BorrowMut as _Borrow,
    collections::HashMap,
    env,
    error::Error as _Error,
    fs, io,
    path::{Path, PathBuf},
    result,
};

use clap::crate_name;
use indexmap::IndexMap;
use log::{debug, info, warn};
use maplit::hashmap;
use serde_derive::{Deserialize, Serialize};

pub use error::{Error, ErrorKind, Result};
pub use logging::init_logging;

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
    Git { url: String },
    /// A GitHub repository, only the the username/repository needs to be
    /// specified.
    GitHub { repository: String },
    /// A local directory.
    Local { directory: PathBuf },
    /// Hints that destructuring should not be exhaustive.
    // Until https://github.com/rust-lang/rust/issues/44109 is stabilized.
    #[serde(skip)]
    #[doc(hidden)]
    __Nonexhaustive,
}

/// The source type for a [`NormalizedPlugin`].
///
/// [`NormalizedPlugin`]: struct.NormalizedPlugin.html
#[derive(Clone, Debug, PartialEq)]
pub enum NormalizedSource {
    /// A clonable Git repository.
    Git { url: String },
    /// A local directory.
    Local,
}

/// A configured shell plugin.
// Note: we would want to use #[serde(deny_unknown_fields)] here but it doesn't
// work with a flattened internally-tagged enum.
// See https://github.com/serde-rs/serde/issues/1358.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Plugin {
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
pub struct NormalizedPlugin {
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
pub struct LockedPlugin {
    /// The name of this plugin.
    name: String,
    /// The directory that this plugin resides in.
    directory: PathBuf,
    /// The filenames to use in the directory.
    filenames: Vec<PathBuf>,
    /// What templates to apply to each filename..
    apply: Vec<String>,
}

/// The contents of a configuration file.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    matches: Vec<String>,
    /// The default list of template names to apply to each matched file.
    apply: Vec<String>,
    /// A map of name to template string.
    templates: HashMap<String, String>,
    /// Each configured plugin.
    plugins: IndexMap<String, Plugin>,
}

/// A locked configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LockedConfig {
    /// The root folder used.
    root: PathBuf,
    /// A map of name to template.
    templates: HashMap<String, String>,
    /// Each locked plugin.
    plugins: Vec<LockedPlugin>,
}

/////////////////////////////////////////////////////////////////////////
// Configuration implementation
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
    /// For a Git or GitHub source this is the path to the repository's
    /// directory where the repository is cloned. For a local source this is
    /// simply the directory defined.
    ///
    /// [`Source`]: enum.Source.html
    pub fn directory(&self, root: &Path) -> Result<PathBuf> {
        let root = root.to_str().unwrap();

        Ok(match self {
            Source::Git { url } => {
                let error = || Error::config_git(url);

                // Parse the URL and generate a folder based on the URL.
                let parsed = url::Url::parse(url).unwrap();
                let base = vec![root, CLONE_DIRECTORY, parsed.host_str().ok_or_else(error)?];
                let segments: Vec<_> = parsed.path_segments().ok_or_else(error)?.collect();
                base.iter().chain(segments.iter()).collect()
            },
            Source::GitHub { repository } => {
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
            _ => unreachable!(),
        })
    }

    /// Return the URL for this [`Source`].
    ///
    /// For a Git or GitHub source this is the URL to the remote repository, for
    /// a local source this is `None`.
    ///
    /// [`Source`]: enum.Source.html
    pub fn url(&self) -> Option<String> {
        match self {
            Source::Git { url } => Some(url.clone()),
            Source::GitHub { repository } => {
                Some(format!("https://{}/{}", GITHUB_HOST, repository))
            },
            Source::Local { .. } => None,
            _ => unreachable!(),
        }
    }

    /// Normalize this [`Source`].
    ///
    /// [`Source`]: enum.Source.html
    fn normalized(self) -> NormalizedSource {
        match self {
            Source::Git { .. } | Source::GitHub { .. } => NormalizedSource::Git {
                url: self.url().unwrap(),
            },
            Source::Local { .. } => NormalizedSource::Local,
            _ => unreachable!(),
        }
    }
}

impl Plugin {
    /// Constructs a new Plugin with the given [`Source`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::{Source, Plugin};
    /// #
    /// let plugin = Plugin::new(
    ///     Source::GitHub {
    ///         repository: "zsh-users/zsh-autosuggestions".to_string()
    ///     }
    /// );
    /// ```
    ///
    /// [`Source`]: enum.Source.html
    pub fn new(source: Source) -> Self {
        Plugin {
            source,
            uses: None,
            apply: None,
        }
    }

    /// Convenience method to construct a new [`Git`] Plugin.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Plugin;
    /// #
    /// let plugin = Plugin::new_git("https://github.com/zsh-users/zsh-autosuggestions");
    /// ```
    /// [`Git`]: enum.Source.html#variant.Git
    pub fn new_git<S: Into<String>>(url: S) -> Self {
        Self::new(Source::Git { url: url.into() })
    }

    /// Convenience method to construct a new [`GitHub`] Plugin.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Plugin;
    /// #
    /// let plugin = Plugin::new_github("zsh-users/zsh-autosuggestions");
    /// ```
    /// [`GitHub`]: enum.Source.html#variant.GitHub
    pub fn new_github<S: Into<String>>(repository: S) -> Self {
        Self::new(Source::GitHub {
            repository: repository.into(),
        })
    }

    /// Convenience method to construct a new [`Local`] Plugin.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Plugin;
    /// #
    /// let plugin = Plugin::new_local("/usr/share/zsh/zsh-autosuggestions");
    /// ```
    /// [`Local`]: enum.Source.html#variant.Local
    pub fn new_local<P: Into<PathBuf>>(directory: P) -> Self {
        Self::new(Source::Local {
            directory: directory.into(),
        })
    }

    /// Set which files to use in this plugin's directory. If none are set
    /// then the files to use will be figured out based on the global
    /// [`matches`] field.
    ///
    /// This can be called multiple times to add more files to use. Valid values
    /// included glob patterns and/or strings containing template fields.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Plugin;
    /// #
    /// let plugin = Plugin::new_github("zsh-users/zsh-autosuggestions")
    ///     .uses("*.zsh")
    ///     .uses("{{ name }}.plugin.zsh");
    /// ```
    ///
    /// [`matches`]: struct.Config.html#method.matches
    pub fn uses<S: Into<String>>(mut self, uses: S) -> Self {
        if let Some(v) = self.uses.borrow_mut() {
            v.push(uses.into());
        } else {
            self.uses = Some(vec_into![uses]);
        }
        self
    }

    /// Add a template to apply to this Plugin. If none are set
    /// then the templates specified in the global [`apply`] field will be
    /// applied.
    ///
    /// This can be called multiple times to add more templates to apply. The
    /// value should be a template name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Plugin;
    /// #
    /// let plugin = Plugin::new_github("zsh-users/zsh-autosuggestions")
    ///     .apply("source")
    ///     .apply("PATH");
    /// ```
    ///
    /// [`apply`]: struct.Config.html#method.apply
    pub fn apply<S: Into<String>>(mut self, apply: S) -> Self {
        if let Some(v) = self.apply.borrow_mut() {
            v.push(apply.into());
        } else {
            self.apply = Some(vec_into![apply]);
        }
        self
    }

    /// Consume the Plugin and convert to a [`NormalizedPlugin`].
    ///
    /// [`NormalizedPlugin`]: struct.NormalizedPlugin.html
    fn normalized(
        self,
        name: String,
        root: &Path,
        apply: &Vec<String>,
    ) -> Result<NormalizedPlugin> {
        Ok(NormalizedPlugin {
            name,
            directory: self.source.directory(root)?,
            source: self.source.normalized(),
            uses: self.uses,
            apply: self.apply.unwrap_or_else(|| apply.clone()),
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
    fn lock(self, root: &Path, matches: &Vec<String>) -> Result<LockedPlugin> {
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

impl Config {
    /// Read a Config from the given path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use sheldon::Config;
    /// #
    /// let config = Config::from_path("plugins.toml");
    /// ```
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let manager = toml::from_str(&String::from_utf8_lossy(
            &fs::read(&path).map_err(|e| Error::deserialize(e, &path))?,
        ))
        .map_err(|e| Error::deserialize(e, &path))?;
        debug!("deserialized config from `{}`", path.to_string_lossy());
        Ok(manager)
    }

    /// Construct a new empty Config using the default values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Config;
    /// #
    /// let config = Config::new();
    /// ```
    pub fn new() -> Self {
        Config::default()
    }

    /// Set the default matches.
    ///
    /// This should be a list of glob patterns. This is slightly different to a
    /// plugin's [`uses`] field, in that this one only uses the first glob
    /// that returns more than zero files.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Config;
    /// #
    /// let config = Config::new()
    ///     .matches(vec![
    ///         "{{ name }}.plugin.zsh".to_string(),
    ///         "*.zsh".to_string()
    ///     ]);
    /// ```
    ///
    /// [`uses`]: struct.Plugin.html#method.uses
    pub fn matches(mut self, matches: Vec<String>) -> Self {
        self.matches = matches;
        self
    }

    /// Set the default templates to apply.
    ///
    /// This should be a list of template names.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Config;
    /// #
    /// let config = Config::new().apply(vec!["source".to_string()]);
    /// ```
    pub fn apply(mut self, apply: Vec<String>) -> Self {
        self.apply = apply;
        self
    }

    /// Add a template string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sheldon::Config;
    /// #
    /// let config = Config::new().template("source", "source \"{{ filename }}\"");
    /// ```
    pub fn template<S: Into<String>, T: Into<String>>(mut self, name: S, template: T) -> Self {
        self.templates.insert(name.into(), template.into());
        self
    }

    /// Add a plugin to this Config.
    pub fn plugin<S: Into<String>>(mut self, name: S, plugin: Plugin) -> Self {
        self.plugins.insert(name.into(), plugin);
        self
    }

    /// Download all required dependencies for plugins.
    ///
    /// TODO: Download in parallel
    fn download(plugins: &Vec<NormalizedPlugin>) -> Result<()> {
        for plugin in plugins {
            if let NormalizedSource::Git { url } = &plugin.source {
                if let Err(e) = git2::Repository::clone(&url, &plugin.directory) {
                    if e.code() != git2::ErrorCode::Exists {
                        return Err(Error::download(e, &url));
                    } else {
                        info!("{} is already cloned (required for `{}`)", url, plugin.name);
                    }
                } else {
                    info!("{} cloned (required for `{}`)", url, plugin.name);
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
            normalized_plugins.push(plugin.normalized(name, root, &self.apply)?);
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
            "source" => "source \"{{ filename }}\""
        };

        // Add custom user templates.
        for (name, template) in &self.templates {
            templates.insert(name.to_string(), template.to_string());
        }

        // Check that the templates can be compiled.
        {
            let mut templates_ = handlebars::Handlebars::new();
            templates_.set_strict_mode(true);
            for (name, template) in &templates {
                templates_
                    .register_template_string(&name, template)
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
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
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
    pub fn new(root: &Path) -> Self {
        Config::new()
            .lock(root)
            .expect("failed to lock default Config")
    }

    /// Write a LockedConfig to the given path.
    pub fn to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
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
    pub fn source(&self) -> Result<String> {
        // Compile the templates
        let mut templates = handlebars::Handlebars::new();
        templates.set_strict_mode(true);
        for (name, template) in &self.templates {
            templates
                .register_template_string(&name, template)
                .map_err(|e| Error::template(e, name))?;
        }

        let mut script = String::new();

        for plugin in &self.plugins {
            for name in &plugin.apply {
                // Data to use in template rendering
                let mut data = hashmap! {
                    "root" =>
                        self.root.to_str().expect("root directory is not valid UTF-8"),
                    "name" =>
                        &plugin.name,
                    "directory" =>
                        plugin.directory.to_str().expect("plugin directory is not valid UTF-8"),
                };

                for filename in &plugin.filenames {
                    data.insert("filename", filename.to_str().unwrap());
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

    /// Create new Context with the default values.
    pub fn new() -> Self {
        Self::defaults(None, None)
    }
}

/// Prepare a configuration for sourcing.
///
/// - Reads the config from the config file.
/// - Downloads all plugin dependencies.
/// - Generates a locked config.
/// - Writes the locked config to the lock file.
pub fn lock(ctx: &Context) -> Result<()> {
    Ok(Config::from_path(&ctx.config_file)?
        .lock(&ctx.root)?
        .to_path(&ctx.lock_file)?)
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
pub fn source(ctx: &Context) -> Result<()> {
    let locked = if config_file_newer(ctx) {
        info!("using new config");
        Config::from_path(&ctx.config_file)?.lock(&ctx.root)?
    } else {
        match LockedConfig::from_path(&ctx.lock_file) {
            Ok(locked) => locked,
            Err(e) => {
                warn!("{}", e);
                match Config::from_path(&ctx.config_file) {
                    Ok(config) => config.lock(&ctx.root)?,
                    Err(e) => {
                        let source = e.source().and_then(|e| e.downcast_ref::<io::Error>());

                        if source.is_some() && source.unwrap().raw_os_error() == Some(2) {
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

    locked.to_path(&ctx.lock_file)?;
    print!("{}", locked.source()?);
    Ok(())
}
