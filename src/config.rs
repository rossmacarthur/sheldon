//! Plugin configuration.
//!
//! This module handles the defining and deserialization of the config file.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::result;
use std::str::{self, FromStr};

use anyhow::{anyhow, bail, Context as ResultExt, Error, Result};
use indexmap::IndexMap;
use regex_macro::regex;
use serde::{self, de, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use url::Url;

/// The Gist domain host.
const GIST_HOST: &str = "gist.github.com";

/// The GitHub domain host.
const GITHUB_HOST: &str = "github.com";

/////////////////////////////////////////////////////////////////////////
// Configuration definitions
/////////////////////////////////////////////////////////////////////////

/// The type of shell that we are using.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shell {
    Bash,
    Zsh,
}

/// A wrapper around a template string.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Template {
    /// The actual template string.
    pub value: String,
    /// Whether this template should be applied to each file.
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
    /// From the tip of a branch.
    Branch(String),
    /// From a specific revision.
    Rev(String),
    /// From a tag.
    Tag(String),
}

/// A Gist repository identifier.
#[derive(Debug, PartialEq)]
pub struct GistRepository {
    /// The GitHub username / organization.
    owner: Option<String>,
    /// The Gist identifier.
    identifier: String,
}

/// A GitHub repository identifier.
#[derive(Debug, PartialEq)]
pub struct GitHubRepository {
    /// The GitHub username / organization.
    owner: String,
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
    Local { dir: PathBuf },
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
    pub proto: Option<GitProtocol>,
    /// The Git reference to checkout.
    #[serde(flatten)]
    pub reference: Option<GitReference>,
    /// Which directory to use in this plugin.
    ///
    /// This directory can contain template parameters.
    pub dir: Option<String>,
    /// Which files to use in this plugin's directory. If this is `None` then
    /// this will figured out based on the global `matches` field.
    ///
    /// These files can contain template parameters.
    #[serde(rename = "use")]
    pub uses: Option<Vec<String>>,
    /// What templates to apply to each matched file. If this is `None` then the
    /// default templates will be applied.
    pub apply: Option<Vec<String>>,
    /// Any extra keys,
    #[serde(flatten, deserialize_with = "deserialize_rest_toml_value")]
    pub rest: Option<toml::Value>,
}

/// An external configured plugin.
#[derive(Debug, PartialEq)]
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
    /// What type of shell is being used.
    pub shell: Option<Shell>,
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    matches: Option<Vec<String>>,
    /// The default list of template names to apply to each matched file.
    apply: Option<Vec<String>>,
    /// A map of name to template string.
    templates: IndexMap<String, Template>,
    /// A map of name to plugin.
    pub plugins: IndexMap<String, RawPlugin>,
    /// Any extra keys,
    #[serde(flatten, deserialize_with = "deserialize_rest_toml_value")]
    pub rest: Option<toml::Value>,
}

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
    pub templates: IndexMap<String, Template>,
    /// Each configured plugin.
    pub plugins: Vec<Plugin>,
}

/////////////////////////////////////////////////////////////////////////
// Serialization implementations
/////////////////////////////////////////////////////////////////////////

impl fmt::Display for Shell {
    /// Displays a `Shell`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Bash => f.write_str("bash"),
            Self::Zsh => f.write_str("zsh"),
        }
    }
}

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
    /// Displays a `GistRepository` as "[{owner}/]{identifier}".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self {
                owner: Some(owner),
                identifier,
            } => write!(f, "{}/{}", owner, identifier),
            Self {
                owner: None,
                identifier,
            } => write!(f, "{}", identifier),
        }
    }
}

impl fmt::Display for GitHubRepository {
    /// Displays a `GitHubRepository` as "{owner}/{repository}".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
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

impl_serialize_as_str!(Shell);
impl_serialize_as_str!(GitProtocol);
impl_serialize_as_str!(GistRepository);
impl_serialize_as_str!(GitHubRepository);

/////////////////////////////////////////////////////////////////////////
// Deserialization implementations
/////////////////////////////////////////////////////////////////////////

impl Default for Shell {
    fn default() -> Self {
        Self::Zsh
    }
}

#[derive(Debug, Error)]
#[error("expected one of `bash` or `zsh`, got `{}`", self.0)]
pub struct ParseShellError(String);

impl str::FromStr for Shell {
    type Err = ParseShellError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match &*s.to_lowercase() {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            s => Err(ParseShellError(s.to_string())),
        }
    }
}

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

#[derive(Debug, Error)]
#[error("expected one of `git`, `https`, or `ssh`, got `{}`", self.0)]
pub struct ParseGitProtocolError(String);

impl FromStr for GitProtocol {
    type Err = ParseGitProtocolError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "git" => Ok(Self::Git),
            "https" => Ok(Self::Https),
            "ssh" => Ok(Self::Ssh),
            s => Err(ParseGitProtocolError(s.to_string())),
        }
    }
}

#[derive(Debug, Error)]
#[error("`{}` is not a valid Gist identifier, the hash or username/hash should be provided", self.0)]
pub struct ParseGistRepositoryError(String);

impl FromStr for GistRepository {
    type Err = ParseGistRepositoryError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        let re = regex!("^((?P<owner>[a-zA-Z0-9_-]+)/)?(?P<identifier>[a-fA-F0-9]+)$");
        let captures = re
            .captures(s)
            .ok_or_else(|| ParseGistRepositoryError(s.to_string()))?;
        let owner = captures.name("owner").map(|m| m.as_str().to_string());
        let identifier = captures.name("identifier").unwrap().as_str().to_string();
        Ok(Self { owner, identifier })
    }
}

#[derive(Debug, Error)]
#[error("`{}` is not a valid GitHub repository, the username/repository should be provided", self.0)]
pub struct ParseGitHubRepositoryError(String);

impl FromStr for GitHubRepository {
    type Err = ParseGitHubRepositoryError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        let re = regex!("^(?P<owner>[a-zA-Z0-9_-]+)/(?P<name>[a-zA-Z0-9\\._-]+)$");
        let captures = re
            .captures(s)
            .ok_or_else(|| ParseGitHubRepositoryError(s.to_string()))?;
        let owner = captures.name("owner").unwrap().as_str().to_string();
        let name = captures.name("name").unwrap().as_str().to_string();
        Ok(Self { owner, name })
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

impl_deserialize_from_str!(shell, Shell, "a supported shell type");
impl_deserialize_from_str!(git_protocol, GitProtocol, "a Git protocol type");
impl_deserialize_from_str!(gist_repository, GistRepository, "a Gist identifier");
impl_deserialize_from_str!(github_repository, GitHubRepository, "a GitHub repository");

/// Deserialize the remaining keys into a `Option<toml::Value>`. Empty tables
/// are coerced to `None`.
fn deserialize_rest_toml_value<'de, D>(deserializer: D) -> Result<Option<toml::Value>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value: toml::Value = de::Deserialize::deserialize(deserializer)?;
    Ok(match value {
        toml::Value::Table(table) => {
            if table.is_empty() {
                None
            } else {
                Some(toml::Value::Table(table))
            }
        }
        value => Some(value),
    })
}

/////////////////////////////////////////////////////////////////////////
// Normalization implementations
/////////////////////////////////////////////////////////////////////////

/// Pop the TOML value from the table, if it is parseable.
fn pop_toml_value<T>(rest: &mut Option<toml::Value>, key: &str) -> Option<T>
where
    T: FromStr,
{
    match rest {
        Some(toml::Value::Table(table)) => match table.get(key) {
            Some(toml::Value::String(s)) => {
                let result = s.parse().ok();
                if result.is_some() {
                    table.remove(key);
                }
                result
            }
            Some(_) | None => None,
        },
        Some(_) => unreachable!(), // unreachable because we are using #[serde(flatten)]
        None => None,
    }
}

/// Check for extra TOML keys, and if they exist then call the given function on
/// the key.
fn check_extra_toml<F>(rest: Option<toml::Value>, mut f: F)
where
    F: FnMut(&str),
{
    match rest {
        Some(toml::Value::Table(table)) => {
            for key in table.keys() {
                f(key)
            }
        }
        Some(_) => unreachable!(), // unreachable because we are using #[serde(flatten)]
        None => {}
    }
}

/// Check whether the specifed templates actually exist.
fn validate_template_names(
    shell: &Shell,
    apply: &Option<Vec<String>>,
    templates: &IndexMap<String, Template>,
) -> Result<()> {
    if let Some(apply) = apply {
        for name in apply {
            if !shell.default_templates().contains_key(name) && !templates.contains_key(name) {
                bail!("unknown template `{}`", name);
            }
        }
    }
    Ok(())
}

impl Template {
    /// Set whether this `Template` should be applied to every file.
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
        matches!(*self, Self::Git { .. })
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
    pub fn normalize(
        self,
        name: String,
        shell: &Shell,
        templates: &IndexMap<String, Template>,
        warnings: &mut Vec<Error>,
    ) -> Result<Plugin> {
        let Self {
            git,
            gist,
            github,
            remote,
            local,
            inline,
            mut proto,
            reference,
            mut dir,
            uses,
            apply,
            mut rest,
        } = self;

        let is_reference_some = reference.is_some();
        let is_gist_or_github = gist.is_some() || github.is_some();

        // Handle some deprecated items :/
        if proto.is_none() {
            if let Some(protocol) = pop_toml_value(&mut rest, "protocol") {
                warnings.push(anyhow!(
                    "use of deprecated config key: `plugins.{name}.protocol`, please use \
                     `plugins.{name}.proto` instead",
                    name = name,
                ));
                proto = Some(protocol);
            }
        }
        if dir.is_none() {
            if let Some(directory) = pop_toml_value(&mut rest, "directory") {
                warnings.push(anyhow!(
                    "deprecated config key used: `plugins.{name}.directory`, please use \
                     `plugins.{name}.dir` instead",
                    name = name,
                ));
                dir = Some(directory);
            }
        }

        check_extra_toml(rest, |key| {
            warnings.push(anyhow!("unused config key: `plugins.{}.{}`", name, key))
        });

        let raw_source = match (git, gist, github, remote, local, inline) {
            // `git` type
            (Some(url), None, None, None, None, None) => {
                TempSource::External(Source::Git { url, reference })
            }
            // `gist` type
            (None, Some(repository), None, None, None, None) => {
                let url_str = format!(
                    "{}{}/{}",
                    proto.unwrap_or(GitProtocol::Https).prefix(),
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
                    proto.unwrap_or(GitProtocol::Https).prefix(),
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
            (None, None, None, None, Some(dir), None) => {
                TempSource::External(Source::Local { dir })
            }
            // `inline` type
            (None, None, None, None, None, Some(raw)) => TempSource::Inline(raw),
            (None, None, None, None, None, None) => {
                bail!("plugin `{}` has no source fields", name);
            }
            _ => {
                bail!("plugin `{}` has multiple source fields", name);
            }
        };

        match raw_source {
            TempSource::External(source) => {
                if !source.is_git() && is_reference_some {
                    bail!(
                        "the `branch`, `tag`, and `rev` fields are not supported by this plugin \
                         type"
                    );
                } else if proto.is_some() && !is_gist_or_github {
                    bail!("the `proto` field is not supported by this plugin type");
                }

                validate_template_names(shell, &apply, templates)?;

                Ok(Plugin::External(ExternalPlugin {
                    name,
                    source,
                    dir,
                    uses,
                    apply,
                }))
            }
            TempSource::Inline(raw) => {
                let unsupported = [
                    ("`proto` field is", proto.is_some()),
                    ("`branch`, `tag`, and `rev` fields are", is_reference_some),
                    ("`dir` field is", dir.is_some()),
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
    const DEPRECATED: &'static [(&'static str, &'static str)] = &[
        ("root", "data_dir"),
        ("directory", "dir"),
        ("filename", "file"),
    ];

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
    fn normalize(self, mut warnings: &mut Vec<Error>) -> Result<Config> {
        let Self {
            shell,
            matches,
            apply,
            templates,
            plugins,
            rest,
        } = self;

        check_extra_toml(rest, |key| {
            warnings.push(anyhow!("unused config key: `{}`", key))
        });

        for (name, template) in &templates {
            // Check that the templates can be compiled.
            handlebars::Template::compile(&template.value)
                .with_context(s!("failed to compile template `{}`", name))?;
            // Simplistic check for deprecated template variables.
            let replaced = template.value.replace(" ", "");
            for (old, new) in Self::DEPRECATED {
                let to_check = ["{{", old, "}}"].concat();
                if replaced.contains(&to_check) {
                    warnings.push(anyhow!(
                        "deprecated template variable used in `templates.{}`: `{}`, please use \
                         `{}` instead",
                        name,
                        old,
                        new,
                    ));
                }
            }
        }

        let shell = shell.unwrap_or_default();

        validate_template_names(&shell, &apply, &templates)?;

        // Normalize the plugins.
        let mut normalized_plugins = Vec::with_capacity(plugins.len());

        for (name, plugin) in plugins {
            normalized_plugins.push(
                plugin
                    .normalize(name.clone(), &shell, &templates, &mut warnings)
                    .with_context(s!("failed to normalize plugin `{}`", name))?,
            );
        }

        Ok(Config {
            shell,
            matches,
            apply,
            templates,
            plugins: normalized_plugins,
        })
    }
}

impl Config {
    /// Read a `Config` from the given path.
    pub fn from_path<P>(path: P, mut warnings: &mut Vec<Error>) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(RawConfig::from_path(path)?.normalize(&mut warnings)?)
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn shell_to_string() {
        assert_eq!(Shell::Bash.to_string(), "bash");
        assert_eq!(Shell::Zsh.to_string(), "zsh");
    }

    #[test]
    fn gist_repository_to_string() {
        let test = GistRepository {
            owner: None,
            identifier: "579d02802b1cc17baed07753d09f5009".to_string(),
        };
        assert_eq!(test.to_string(), "579d02802b1cc17baed07753d09f5009");
    }

    #[test]
    fn gist_repository_to_string_with_owner() {
        let test = GistRepository {
            owner: Some("rossmacarthur".to_string()),
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
            owner: "rossmacarthur".to_string(),
            name: "sheldon-test".to_string(),
        };
        assert_eq!(test.to_string(), "rossmacarthur/sheldon-test");
    }

    #[derive(Debug, Deserialize)]
    struct ShellTest {
        s: Shell,
    }

    #[test]
    fn shell_deserialize_as_str() {
        let test: ShellTest = toml::from_str("s = 'bash'").unwrap();
        assert_eq!(test.s, Shell::Bash)
    }

    #[test]
    fn shell_deserialize_invalid() {
        let error = toml::from_str::<ShellTest>("s = 'ksh'").unwrap_err();
        assert_eq!(
            error.to_string(),
            "expected one of `bash` or `zsh`, got `ksh` for key `s` at line 1 column 5"
        )
    }

    #[derive(Debug, Deserialize)]
    struct TemplateTest {
        t: Template,
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

    #[derive(Debug, Deserialize)]
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
    fn git_reference_deserialize_rev() {
        let test: TestGitReference = toml::from_str("rev = 'cd65e828'").unwrap();
        assert_eq!(test.g, GitReference::Rev(String::from("cd65e828")));
    }

    #[derive(Debug, Deserialize)]
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
                owner: Some("rossmacarthur".to_string()),
                identifier: "579d02802b1cc17baed07753d09f5009".to_string()
            }
        );
    }

    #[test]
    fn gist_repository_deserialize_two_slashes() {
        let error = toml::from_str::<TestGistRepository>(
            "g = 'rossmacarthur/579d02802b1cc17baed07753d09f5009/test'",
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "`rossmacarthur/579d02802b1cc17baed07753d09f5009/test` is not a valid Gist \
             identifier, the hash or username/hash should be provided for key `g` at line 1 \
             column 5"
        );
    }

    #[test]
    fn gist_repository_deserialize_not_hex() {
        let error = toml::from_str::<TestGistRepository>("g = 'nothex'").unwrap_err();
        assert_eq!(
            error.to_string(),
            "`nothex` is not a valid Gist identifier, the hash or username/hash should be \
             provided for key `g` at line 1 column 5"
        );
    }

    #[derive(Debug, Deserialize)]
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
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string()
            }
        );
    }

    #[test]
    fn github_repository_deserialize_two_slashes() {
        let error =
            toml::from_str::<TestGitHubRepository>("g = 'rossmacarthur/sheldon/test'").unwrap_err();
        assert_eq!(
            error.to_string(),
            "`rossmacarthur/sheldon/test` is not a valid GitHub repository, the \
             username/repository should be provided for key `g` at line 1 column 5"
        );
    }

    #[test]
    fn github_repository_deserialize_no_slashes() {
        let error = toml::from_str::<TestGitHubRepository>("g = 'noslash'").unwrap_err();
        assert_eq!(
            error.to_string(),
            "`noslash` is not a valid GitHub repository, the username/repository should be \
             provided for key `g` at line 1 column 5"
        );
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
                owner: "rossmacarthur".into(),
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
                    .normalize(
                        "test".to_string(),
                        &Shell::default(),
                        &IndexMap::new(),
                        &mut Vec::new(),
                    )
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            git: Some(url),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some(
                "rossmacarthur/579d02802b1cc17baed07753d09f5009"
                    .parse()
                    .unwrap(),
            ),
            proto: Some(GitProtocol::Git),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some("579d02802b1cc17baed07753d09f5009".parse().unwrap()),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some(
                "rossmacarthur/579d02802b1cc17baed07753d09f5009"
                    .parse()
                    .unwrap(),
            ),
            proto: Some(GitProtocol::Ssh),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            proto: Some(GitProtocol::Git),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            proto: Some(GitProtocol::Ssh),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            remote: Some(url),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            .normalize(
                "test".to_string(),
                &Shell::default(),
                &IndexMap::new(),
                &mut Vec::new(),
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the `branch`, `tag`, and `rev` fields are not supported by this plugin type"
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
            proto: Some(GitProtocol::Https),
            ..Default::default()
        };
        let error = raw_plugin
            .normalize(
                "test".to_string(),
                &Shell::default(),
                &IndexMap::new(),
                &mut Vec::new(),
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the `proto` field is not supported by this plugin type"
        );
    }

    #[test]
    fn raw_plugin_normalize_local() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Local {
                dir: "/home/temp".into(),
            },
            dir: None,
            uses: None,
            apply: None,
        });
        let raw_plugin = RawPlugin {
            local: Some("/home/temp".into()),
            ..Default::default()
        };
        assert_eq!(
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            raw_plugin
                .normalize(name, &Shell::default(), &IndexMap::new(), &mut Vec::new())
                .unwrap(),
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
            .normalize(
                "test".to_string(),
                &Shell::default(),
                &IndexMap::new(),
                &mut Vec::new(),
            )
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
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            apply: Some(vec_into!["test"]),
            ..Default::default()
        };
        let error = raw_plugin
            .normalize(
                "test".to_string(),
                &Shell::default(),
                &IndexMap::new(),
                &mut Vec::new(),
            )
            .unwrap_err();
        assert_eq!(error.to_string(), "unknown template `test`");
    }
}
