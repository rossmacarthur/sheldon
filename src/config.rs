//! Plugin configuration.
//!
//! This module handles the defining and deserialization of the config file.

use std::{
    error, fmt, fs,
    path::{Path, PathBuf},
    result, str,
    str::FromStr,
};

use anyhow::{bail, Context as ResultExt, Result};
use indexmap::IndexMap;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{self, de, Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

use crate::lock::DEFAULT_TEMPLATES;

/// The Gist domain host.
const GIST_HOST: &str = "gist.github.com";

/// The GitHub domain host.
const GITHUB_HOST: &str = "github.com";

/////////////////////////////////////////////////////////////////////////
// Configuration definitions
/////////////////////////////////////////////////////////////////////////

/// A wrapper around a template string.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Template {
    /// The actual template string.
    pub value: String,
    /// Whether this template should be applied to each filename.
    pub each: bool,
}

/// The Git protocol.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GitProtocol {
    Git,
    Https,
    Ssh,
}

/// A Git reference.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitReference {
    /// From the HEAD of a branch.
    Branch(String),
    /// From a specific revision.
    Revision(String),
    /// From a tag.
    Tag(String),
}

/// A Gist repository identifier.
#[derive(Debug, PartialEq)]
pub struct GistRepository {
    /// The GitHub username / organization.
    vendor: Option<String>,
    /// The Gist identifier.
    identifier: String,
}

/// A GitHub repository identifier.
#[derive(Debug, PartialEq)]
pub struct GitHubRepository {
    /// The GitHub username / organization.
    vendor: String,
    /// The GitHub repository name.
    name: String,
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

/// The actual plugin configuration.
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct RawPlugin {
    /// A clonable Git repository.
    pub git: Option<Url>,
    /// A clonable Gist repository.
    pub gist: Option<GistRepository>,
    /// A clonable GitHub repository.
    pub github: Option<GitHubRepository>,
    /// A downloadable file.
    pub remote: Option<Url>,
    /// A local directory.
    pub local: Option<PathBuf>,
    /// An inline script.
    pub inline: Option<String>,
    /// What protocol to use when cloning a repository.
    pub protocol: Option<GitProtocol>,
    /// The Git reference to checkout.
    #[serde(flatten)]
    pub reference: Option<GitReference>,
    /// Which directory to use in this plugin.
    ///
    /// This directory can contain template parameters.
    pub directory: Option<String>,
    /// Which files to use in this plugin's directory. If this is `None` then
    /// this will figured out based on the global `matches` field.
    ///
    /// These filenames can contain template parameters.
    #[serde(rename = "use")]
    pub uses: Option<Vec<String>>,
    /// What templates to apply to each matched file. If this is `None` then the
    /// default templates will be applied.
    pub apply: Option<Vec<String>>,
}

/// An external configured plugin.
#[derive(Debug, PartialEq)]
pub struct ExternalPlugin {
    /// The name of this plugin.
    pub name: String,
    /// Specifies how to retrieve this plugin.
    pub source: Source,
    /// Which directory to use in this plugin.
    pub directory: Option<String>,
    /// What files to use in the plugin's directory.
    pub uses: Option<Vec<String>>,
    /// What templates to apply to each matched file.
    pub apply: Option<Vec<String>>,
}

/// An inline configured plugin.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct InlinePlugin {
    /// The name of this plugin.
    pub name: String,
    /// The actual source.
    pub raw: String,
}

/// A configured plugin.
#[derive(Debug, PartialEq)]
pub enum Plugin {
    External(ExternalPlugin),
    Inline(InlinePlugin),
}

/// The contents of the configuration file.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawConfig {
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    matches: Option<Vec<String>>,
    /// The default list of template names to apply to each matched file.
    apply: Option<Vec<String>>,
    /// A map of name to template string.
    templates: IndexMap<String, Template>,
    /// A map of name to plugin.
    pub plugins: IndexMap<String, RawPlugin>,
}

/// The user configuration.
#[derive(Debug)]
pub struct Config {
    /// Which files to match and use in a plugin's directory.
    pub matches: Option<Vec<String>>,
    /// The default list of template names to apply to each matched file.
    pub apply: Option<Vec<String>>,
    /// A map of name to template string.
    pub templates: IndexMap<String, Template>,
    /// Each configured plugin.
    pub plugins: Vec<Plugin>,
}

/////////////////////////////////////////////////////////////////////////
// Serialization implementations
/////////////////////////////////////////////////////////////////////////

impl fmt::Display for GitProtocol {
    /// Displays a `GitProtocol`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Git => f.write_str("git"),
            Self::Https => f.write_str("https"),
            Self::Ssh => f.write_str("ssh"),
        }
    }
}

impl fmt::Display for GistRepository {
    /// Displays a `GistRepository` as "[{vendor}/]{identifier}".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self {
                vendor: Some(vendor),
                identifier,
            } => write!(f, "{}/{}", vendor, identifier),
            Self {
                vendor: None,
                identifier,
            } => write!(f, "{}", identifier),
        }
    }
}

impl fmt::Display for GitHubRepository {
    /// Displays a `GitHubRepository` as "{vendor}/{repository}".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.vendor, self.name)
    }
}

macro_rules! impl_serialize_as_str {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }
    };
}

impl_serialize_as_str!(GitProtocol);
impl_serialize_as_str!(GistRepository);
impl_serialize_as_str!(GitHubRepository);

/////////////////////////////////////////////////////////////////////////
// Deserialization implementations
/////////////////////////////////////////////////////////////////////////

/// A visitor to deserialize a `Template` from a string or a struct.
struct TemplateVisitor;

/// The same as a `Template`. It is used to prevent recursion when
/// deserializing.
#[derive(Deserialize)]
struct TemplateAux {
    value: String,
    each: bool,
}

impl From<TemplateAux> for Template {
    fn from(aux: TemplateAux) -> Self {
        let TemplateAux { value, each } = aux;
        Self { value, each }
    }
}

impl From<&str> for Template {
    fn from(s: &str) -> Self {
        Self {
            value: s.to_string(),
            each: false,
        }
    }
}

impl<'de> de::Visitor<'de> for TemplateVisitor {
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
        M: de::MapAccess<'de>,
    {
        let aux: TemplateAux =
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(visitor))?;
        Ok(aux.into())
    }
}

/// Manually implement `Deserialize` for a `Template`.
///
/// Unfortunately we can not use [the recommended method][string-or-struct],
/// because we are storing `Template`s in a map.
///
/// [string-or-struct](https://serde.rs/string-or-struct.html)
impl<'de> Deserialize<'de> for Template {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TemplateVisitor)
    }
}

#[derive(Debug)]
pub struct ParseGitProtocolError;

impl fmt::Display for ParseGitProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected one of `git`, `https`, or `ssh`")
    }
}

impl error::Error for ParseGitProtocolError {}

impl FromStr for GitProtocol {
    type Err = ParseGitProtocolError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "git" => Ok(Self::Git),
            "https" => Ok(Self::Https),
            "ssh" => Ok(Self::Ssh),
            _ => Err(ParseGitProtocolError),
        }
    }
}

macro_rules! make_regex_matcher {
    ($name:ident, $regex:literal) => {
        fn $name(s: &str) -> bool {
            lazy_static! {
                static ref RE: Regex = Regex::new($regex).expect("invalid regex");
            }
            RE.is_match(s)
        }
    };
}

make_regex_matcher!(is_valid_gist_identifier, "^[a-fA-F0-9]+$");
make_regex_matcher!(is_valid_github_vendor, "^[a-zA-Z0-9_-]+$");
make_regex_matcher!(is_valid_github_repository, "^[a-zA-Z0-9\\._-]+$");

#[derive(Debug)]
pub struct ParseGistRepositoryError;

impl fmt::Display for ParseGistRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a valid Gist identifier, the hash or username/hash should be provided")
    }
}

impl error::Error for ParseGistRepositoryError {}

impl FromStr for GistRepository {
    type Err = ParseGistRepositoryError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        let mut s_split = s.rsplit('/');
        let identifier = s_split.next().ok_or(ParseGistRepositoryError)?.to_string();
        let vendor = s_split.next().map(ToString::to_string);

        if s_split.next().is_some() {
            return Err(ParseGistRepositoryError);
        }
        if let Some(vendor) = &vendor {
            if !is_valid_github_vendor(vendor) {
                return Err(ParseGistRepositoryError);
            }
        }
        if !is_valid_gist_identifier(&identifier) {
            return Err(ParseGistRepositoryError);
        }

        Ok(Self { vendor, identifier })
    }
}

#[derive(Debug)]
pub struct ParseGitHubRepositoryError;

impl fmt::Display for ParseGitHubRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a valid GitHub repository, the username/repository should be provided")
    }
}

impl error::Error for ParseGitHubRepositoryError {}

impl FromStr for GitHubRepository {
    type Err = ParseGitHubRepositoryError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        let mut s_split = s.split('/');
        let vendor = s_split
            .next()
            .ok_or(ParseGitHubRepositoryError)?
            .to_string();
        let name = s_split
            .next()
            .ok_or(ParseGitHubRepositoryError)?
            .to_string();

        if s_split.next().is_some() {
            return Err(ParseGitHubRepositoryError);
        }
        if !is_valid_github_vendor(&vendor) || !is_valid_github_repository(&name) {
            return Err(ParseGitHubRepositoryError);
        }

        Ok(Self { vendor, name })
    }
}

macro_rules! impl_deserialize_from_str {
    ($module:ident, $name:ident, $expecting:expr) => {
        mod $module {
            use super::*;

            struct Visitor;

            impl<'de> de::Visitor<'de> for Visitor {
                type Value = $name;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str($expecting)
                }

                fn visit_str<E>(self, value: &str) -> result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    $name::from_str(value).map_err(|e| de::Error::custom(e.to_string()))
                }
            }

            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    deserializer.deserialize_str(Visitor)
                }
            }
        }
    };
}

impl_deserialize_from_str!(git_protocol, GitProtocol, "a Git protocol type");
impl_deserialize_from_str!(gist_repository, GistRepository, "a Gist identifier");
impl_deserialize_from_str!(github_repository, GitHubRepository, "a GitHub repository");

/////////////////////////////////////////////////////////////////////////
// Normalization implementations
/////////////////////////////////////////////////////////////////////////

/// Check whether the specifed templates actually exist.
fn validate_template_names(
    apply: &Option<Vec<String>>,
    templates: &IndexMap<String, Template>,
) -> Result<()> {
    if let Some(apply) = apply {
        for name in apply {
            if !DEFAULT_TEMPLATES.contains_key(name) && !templates.contains_key(name) {
                bail!("unknown template `{}`", name);
            }
        }
    }
    Ok(())
}

impl Template {
    /// Set whether this `Template` should be applied to every filename.
    pub fn each(mut self, each: bool) -> Self {
        self.each = each;
        self
    }
}

impl GitProtocol {
    fn prefix(&self) -> &str {
        match self {
            Self::Git => "git://",
            Self::Https => "https://",
            Self::Ssh => "ssh://git@",
        }
    }
}

impl Source {
    /// Whether the `Source` is a `Source::Git` variant.
    fn is_git(&self) -> bool {
        match *self {
            Self::Git { .. } => true,
            _ => false,
        }
    }
}

/// A convenience struct to help with normalizing the config file in
/// `RawPlugin::normalize()`.
#[derive(Debug)]
enum TempSource {
    External(Source),
    Inline(String),
}

impl RawPlugin {
    /// Normalize a `RawPlugin` into a `Plugin` which is simpler and easier
    /// to handle.
    pub fn normalize(self, name: String, templates: &IndexMap<String, Template>) -> Result<Plugin> {
        let Self {
            git,
            gist,
            github,
            remote,
            local,
            inline,
            protocol,
            reference,
            directory,
            uses,
            apply,
        } = self;

        let is_reference_some = reference.is_some();
        let is_gist_or_github = gist.is_some() || github.is_some();

        let raw_source = match (git, gist, github, remote, local, inline) {
            // `git` type
            (Some(url), None, None, None, None, None) => {
                TempSource::External(Source::Git { url, reference })
            }
            // `gist` type
            (None, Some(repository), None, None, None, None) => {
                let url_str = format!(
                    "{}{}/{}",
                    protocol.unwrap_or(GitProtocol::Https).prefix(),
                    GIST_HOST,
                    repository.identifier
                );
                let url = Url::parse(&url_str)
                    .with_context(s!("failed to construct Gist URL using `{}`", repository))?;
                TempSource::External(Source::Git { url, reference })
            }
            // `github` type
            (None, None, Some(repository), None, None, None) => {
                let url_str = format!(
                    "{}{}/{}",
                    protocol.unwrap_or(GitProtocol::Https).prefix(),
                    GITHUB_HOST,
                    repository
                );
                let url = Url::parse(&url_str)
                    .with_context(s!("failed to construct GitHub URL using `{}`", repository))?;
                TempSource::External(Source::Git { url, reference })
            }
            // `remote` type
            (None, None, None, Some(url), None, None) => {
                TempSource::External(Source::Remote { url })
            }
            // `local` type
            (None, None, None, None, Some(directory), None) => {
                TempSource::External(Source::Local { directory })
            }
            // `inline` type
            (None, None, None, None, None, Some(raw)) => TempSource::Inline(raw),
            _ => {
                bail!("plugin `{}` has multiple source fields", name);
            }
        };

        match raw_source {
            TempSource::External(source) => {
                if !source.is_git() && is_reference_some {
                    bail!(
                        "the `branch`, `tag`, and `revision` fields are not supported by this \
                         plugin type"
                    );
                } else if protocol.is_some() && !is_gist_or_github {
                    bail!("the `protocol` field is not supported by this plugin type");
                }

                validate_template_names(&apply, templates)?;

                Ok(Plugin::External(ExternalPlugin {
                    name,
                    source,
                    directory,
                    uses,
                    apply,
                }))
            }
            TempSource::Inline(raw) => {
                let unsupported = [
                    ("`protocol` field is", protocol.is_some()),
                    (
                        "`branch`, `tag`, and `revision` fields are",
                        is_reference_some,
                    ),
                    ("`directory` field is", directory.is_some()),
                    ("`use` field is", uses.is_some()),
                    ("`apply` field is", apply.is_some()),
                ];
                for (field, is_some) in &unsupported {
                    if *is_some {
                        bail!("the {} not supported by inline plugins", field);
                    }
                }
                Ok(Plugin::Inline(InlinePlugin { name, raw }))
            }
        }
    }
}

impl RawConfig {
    /// Read a `RawConfig` from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let contents = String::from_utf8(
            fs::read(&path).with_context(s!("failed to read from `{}`", path.display()))?,
        )
        .context("config file contents are not valid UTF-8")?;
        let config: Self =
            toml::from_str(&contents).context("failed to deserialize contents as TOML")?;
        Ok(config)
    }

    /// Normalize a `RawConfig` into a `Config`.
    fn normalize(self) -> Result<Config> {
        let Self {
            matches,
            apply,
            templates,
            plugins,
        } = self;

        // Check that the templates can be compiled.
        {
            let mut handlebars = handlebars::Handlebars::new();
            handlebars.set_strict_mode(true);
            for (name, template) in &templates {
                handlebars
                    .register_template_string(&name, &template.value)
                    .with_context(s!("failed to compile template `{}`", name))?
            }
        }

        validate_template_names(&apply, &templates)?;

        // Normalize the plugins.
        let mut normalized_plugins = Vec::with_capacity(plugins.len());

        for (name, plugin) in plugins {
            normalized_plugins.push(
                plugin
                    .normalize(name.clone(), &templates)
                    .with_context(s!("failed to normalize plugin `{}`", name))?,
            );
        }

        Ok(Config {
            matches,
            apply,
            templates,
            plugins: normalized_plugins,
        })
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

    #[derive(Debug, Deserialize)]
    struct TemplateTest {
        t: Template,
    }

    #[test]
    fn gist_repository_to_string() {
        let test = GistRepository {
            vendor: None,
            identifier: "579d02802b1cc17baed07753d09f5009".to_string(),
        };
        assert_eq!(test.to_string(), "579d02802b1cc17baed07753d09f5009");
    }

    #[test]
    fn gist_repository_to_string_with_vendor() {
        let test = GistRepository {
            vendor: Some("rossmacarthur".to_string()),
            identifier: "579d02802b1cc17baed07753d09f5009".to_string(),
        };
        assert_eq!(
            test.to_string(),
            "rossmacarthur/579d02802b1cc17baed07753d09f5009"
        );
    }

    #[test]
    fn github_repository_to_string() {
        let test = GitHubRepository {
            vendor: "rossmacarthur".to_string(),
            name: "sheldon-test".to_string(),
        };
        assert_eq!(test.to_string(), "rossmacarthur/sheldon-test");
    }

    #[test]
    fn template_deserialize_as_str() {
        let test: TemplateTest = toml::from_str("t = 'test'").unwrap();
        assert_eq!(
            test.t,
            Template {
                value: "test".to_string(),
                each: false
            }
        );
    }

    #[test]
    fn template_deserialize_as_map() {
        let test: TemplateTest = toml::from_str("t = { value = 'test', each = true }").unwrap();
        assert_eq!(
            test.t,
            Template {
                value: "test".to_string(),
                each: true
            }
        );
    }

    #[test]
    fn template_deserialize_invalid() {
        let error = toml::from_str::<TemplateTest>("t = 0").unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid type: integer `0`, expected string or map for key `t` at line 1 column 5"
        );
    }

    #[derive(Deserialize)]
    struct TestGitReference {
        #[serde(flatten)]
        g: GitReference,
    }

    #[test]
    fn git_reference_deserialize_branch() {
        let test: TestGitReference = toml::from_str("branch = 'master'").unwrap();
        assert_eq!(test.g, GitReference::Branch(String::from("master")));
    }

    #[test]
    fn git_reference_deserialize_tag() {
        let test: TestGitReference = toml::from_str("tag = 'v0.5.1'").unwrap();
        assert_eq!(test.g, GitReference::Tag(String::from("v0.5.1")));
    }

    #[test]
    fn git_reference_deserialize_revision() {
        let test: TestGitReference = toml::from_str("revision = 'cd65e828'").unwrap();
        assert_eq!(test.g, GitReference::Revision(String::from("cd65e828")));
    }

    #[derive(Deserialize)]
    struct TestGistRepository {
        g: GistRepository,
    }

    #[test]
    fn gist_repository_deserialize() {
        let test: TestGistRepository =
            toml::from_str("g = 'rossmacarthur/579d02802b1cc17baed07753d09f5009'").unwrap();
        assert_eq!(
            test.g,
            GistRepository {
                vendor: Some("rossmacarthur".to_string()),
                identifier: "579d02802b1cc17baed07753d09f5009".to_string()
            }
        );
    }

    #[test]
    #[should_panic]
    fn gist_repository_deserialize_two_slashes() {
        toml::from_str::<TestGistRepository>(
            "g = 'rossmacarthur/579d02802b1cc17baed07753d09f5009/test'",
        )
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn gist_repository_deserialize_not_hex() {
        toml::from_str::<TestGistRepository>("g = 'nothex'").unwrap();
    }

    #[derive(Deserialize)]
    struct TestGitHubRepository {
        g: GitHubRepository,
    }

    #[test]
    fn github_repository_deserialize() {
        let test: TestGitHubRepository =
            toml::from_str("g = 'rossmacarthur/sheldon-test'").unwrap();
        assert_eq!(
            test.g,
            GitHubRepository {
                vendor: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string()
            }
        );
    }

    #[test]
    #[should_panic]
    fn github_repository_deserialize_two_slashes() {
        toml::from_str::<TestGitHubRepository>("g = 'rossmacarthur/sheldon/test'").unwrap();
    }

    #[test]
    #[should_panic]
    fn github_repository_deserialize_no_slashes() {
        toml::from_str::<TestGitHubRepository>("g = 'noslash'").unwrap();
    }

    #[test]
    fn raw_plugin_deserialize_git() {
        let expected = RawPlugin {
            git: Some(Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap()),
            ..Default::default()
        };
        let plugin: RawPlugin =
            toml::from_str("git = 'https://github.com/rossmacarthur/sheldon-test'").unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn raw_plugin_deserialize_github() {
        let expected = RawPlugin {
            github: Some(GitHubRepository {
                vendor: "rossmacarthur".into(),
                name: "sheldon-test".into(),
            }),
            ..Default::default()
        };
        let plugin: RawPlugin = toml::from_str("github = 'rossmacarthur/sheldon-test'").unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn raw_plugin_deserialize_conflicts() {
        let sources = [
            ("git", "https://github.com/rossmacarthur/sheldon-test"),
            ("gist", "579d02802b1cc17baed07753d09f5009"),
            ("github", "rossmacarthur/sheldon-test"),
            ("remote", "https://ross.macarthur.io"),
            ("local", "~/.dotfiles/zsh/pure"),
            ("inline", "derp"),
        ];

        for (a, example_a) in &sources {
            for (b, example_b) in &sources {
                if a == b {
                    continue;
                }
                let text = format!("{} = '{}'\n{} = '{}'", a, example_a, b, example_b);
                let e = toml::from_str::<RawPlugin>(&text)
                    .unwrap()
                    .normalize("test".to_string(), &IndexMap::new())
                    .unwrap_err();
                assert_eq!(e.to_string(), "plugin `test` has multiple source fields")
            }
        }
    }

    #[test]
    fn raw_plugin_normalize_git() {
        let name = "test".to_string();
        let url = Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: url.clone(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            git: Some(url),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_gist_with_git() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse("git://gist.github.com/579d02802b1cc17baed07753d09f5009").unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some(
                "rossmacarthur/579d02802b1cc17baed07753d09f5009"
                    .parse()
                    .unwrap(),
            ),
            protocol: Some(GitProtocol::Git),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_gist_with_https() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse("https://gist.github.com/579d02802b1cc17baed07753d09f5009")
                    .unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some("579d02802b1cc17baed07753d09f5009".parse().unwrap()),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_gist_with_ssh() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse("ssh://git@gist.github.com/579d02802b1cc17baed07753d09f5009")
                    .unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some(
                "rossmacarthur/579d02802b1cc17baed07753d09f5009"
                    .parse()
                    .unwrap(),
            ),
            protocol: Some(GitProtocol::Ssh),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_github_with_git() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse("git://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                vendor: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            protocol: Some(GitProtocol::Git),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_github_with_https() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                vendor: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_github_with_ssh() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse("ssh://git@github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                vendor: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            protocol: Some(GitProtocol::Ssh),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_remote() {
        let name = "test".to_string();
        let url =
            Url::parse("https://github.com/rossmacarthur/sheldon-test/blob/master/test.plugin.zsh")
                .unwrap();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Remote { url: url.clone() },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            remote: Some(url),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_remote_with_reference() {
        let raw_plugin = RawPlugin {
            remote: Some(
                Url::parse(
                    "https://github.com/rossmacarthur/sheldon-test/blob/master/test.plugin.zsh",
                )
                .unwrap(),
            ),
            reference: Some(GitReference::Tag("v0.1.0".to_string())),
            ..Default::default()
        };
        let error = raw_plugin
            .normalize("test".to_string(), &IndexMap::new())
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the `branch`, `tag`, and `revision` fields are not supported by this plugin type"
        );
    }

    #[test]
    fn raw_plugin_normalize_remote_with_ssh() {
        let raw_plugin = RawPlugin {
            remote: Some(
                Url::parse(
                    "https://github.com/rossmacarthur/sheldon-test/blob/master/test.plugin.zsh",
                )
                .unwrap(),
            ),
            protocol: Some(GitProtocol::Https),
            ..Default::default()
        };
        let error = raw_plugin
            .normalize("test".to_string(), &IndexMap::new())
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the `protocol` field is not supported by this plugin type"
        );
    }

    #[test]
    fn raw_plugin_normalize_local() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Local {
                directory: "/home/temp".into(),
            },
            directory: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            local: Some("/home/temp".into()),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_inline() {
        let name = "test".to_string();
        let expected = Plugin::Inline(InlinePlugin {
            name: name.clone(),
            raw: "echo 'this is a test'\n".to_string(),
        });
        let raw_plugin = RawPlugin {
            inline: Some("echo 'this is a test'\n".to_string()),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin.normalize(name, &IndexMap::new()).unwrap(),
            expected
        );
    }

    #[test]
    fn raw_plugin_normalize_inline_apply() {
        let raw_plugin = RawPlugin {
            inline: Some("echo 'this is a test'\n".to_string()),
            apply: Some(vec_into!["test"]),
            ..Default::default()
        };
        let error = raw_plugin
            .normalize("test".to_string(), &IndexMap::new())
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the `apply` field is not supported by inline plugins"
        );
    }

    #[test]
    fn raw_plugin_normalize_external_invalid_template() {
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                vendor: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            apply: Some(vec_into!["test"]),
            ..Default::default()
        };
        let error = raw_plugin
            .normalize("test".to_string(), &IndexMap::new())
            .unwrap_err();
        assert_eq!(error.to_string(), "unknown template `test`");
    }

    #[test]
    fn config_from_path_example() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("docs/plugins.example.toml");
        Config::from_path(path).unwrap();
    }
}
