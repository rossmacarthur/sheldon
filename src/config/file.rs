//! The config file.

use std::{
    fmt,
    fs,
    path::{Path, PathBuf},
    result,
};

use indexmap::IndexMap;
use serde::{self, de, Deserialize, Deserializer};
use url::Url;

use crate::{
    config::{Config, GitReference, Plugin, Source, Template},
    Error,
    Result,
    ResultExt,
};

/// The Gist domain host.
const GIST_HOST: &str = "gist.github.com";

/// The GitHub domain host.
const GITHUB_HOST: &str = "github.com";

/////////////////////////////////////////////////////////////////////////
// Configuration definitions
/////////////////////////////////////////////////////////////////////////

/// A GitHub repository identifier.
#[derive(Debug, PartialEq)]
struct GitHubRepository {
    /// The GitHub username / organization.
    username: String,
    /// The GitHub repository name.
    repository: String,
}

/// The actual plugin configuration.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RawPluginInner {
    /// A clonable Git repository.
    #[serde(with = "url_serde")]
    git: Option<Url>,
    /// A Gist snippet, only the hash or username/hash needs to be specified.
    gist: Option<String>,
    /// A clonable GitHub repository.
    github: Option<GitHubRepository>,
    /// A downloadable file.
    #[serde(with = "url_serde")]
    remote: Option<Url>,
    /// A local directory.
    local: Option<PathBuf>,
    /// The Git reference to checkout.
    #[serde(flatten)]
    reference: Option<GitReference>,
    /// Which directory to use in this plugin.
    directory: Option<String>,
    /// Which files to use in this plugin's directory. If this is `None` then
    /// this will figured out based on the global `matches` field.
    #[serde(rename = "use")]
    uses: Option<Vec<String>>,
    /// What templates to apply to each matched file. If this is `None` then the
    /// default templates will be applied.
    apply: Option<Vec<String>>,
}

/// The plugin configuration.
///
/// This wraps a `RawPluginInner` so we can add validation to the
/// deserialization.
#[derive(Debug, Default, Deserialize, PartialEq)]
struct RawPlugin {
    #[serde(deserialize_with = "deserialize_raw_plugin_inner", flatten)]
    inner: RawPluginInner,
}

/// The contents of the configuration file.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct RawConfig {
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    matches: Vec<String>,
    /// The default list of template names to apply to each matched file.
    apply: Vec<String>,
    /// A map of name to template string.
    templates: IndexMap<String, Template>,
    /// A map of name to plugin.
    plugins: IndexMap<String, RawPlugin>,
}

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
/// Unfortunately we can not use https://serde.rs/string-or-struct.html,
/// because we are storing `Template`s in a map.
impl<'de> Deserialize<'de> for Template {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TemplateVisitor)
    }
}

impl From<TemplateAux> for Template {
    fn from(aux: TemplateAux) -> Self {
        let TemplateAux { value, each } = aux;
        Template { value, each }
    }
}

impl From<&str> for Template {
    fn from(s: &str) -> Self {
        Template {
            value: s.to_string(),
            each: false,
        }
    }
}

/// A visitor to deserialize a `GitHubRepository` from a string.
struct GitHubRepositoryVisitor;

impl<'de> de::Visitor<'de> for GitHubRepositoryVisitor {
    type Value = GitHubRepository;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("the `<username>/<repository>` GitHub repository identifier")
    }

    fn visit_str<E>(self, value: &str) -> result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        let error = || {
            de::Error::custom(format!(
                "failed to parse `{}` as a GitHub repository",
                value
            ))
        };
        let mut value_split = value.splitn(2, '/');
        let username = value_split.next().ok_or_else(error)?.to_string();
        let repository = value_split.next().ok_or_else(error)?.to_string();

        if repository.contains('/') {
            return Err(error());
        }

        Ok(GitHubRepository {
            username,
            repository,
        })
    }
}

/// Manually implement `Deserialize` for a `GitHubRepository`.
impl<'de> Deserialize<'de> for GitHubRepository {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(GitHubRepositoryVisitor)
    }
}

/// Custom function to deserialize and validate a `RawPluginInner`.
fn deserialize_raw_plugin_inner<'de, D>(deserializer: D) -> result::Result<RawPluginInner, D::Error>
where
    D: Deserializer<'de>,
{
    let inner: RawPluginInner = RawPluginInner::deserialize(deserializer)?;

    if [
        inner.git.is_some(),
        inner.gist.is_some(),
        inner.github.is_some(),
        inner.remote.is_some(),
        inner.local.is_some(),
    ]
    .iter()
    .map(|x| *x as u32)
    .sum::<u32>()
        != 1
    {
        return Err(de::Error::custom("multiple source fields specified"));
    }

    if inner.reference.is_some() && (inner.remote.is_some() || inner.local.is_some()) {
        return Err(de::Error::custom(
            "'reference' field is not valid for source type",
        ));
    }

    Ok(inner)
}

impl Default for RawConfig {
    /// Returns the default `RawConfig`.
    fn default() -> Self {
        RawConfig {
            templates: IndexMap::new(),
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

/////////////////////////////////////////////////////////////////////////
// Normalization implementations
/////////////////////////////////////////////////////////////////////////

impl fmt::Display for GitHubRepository {
    /// Displays a `GitHubRepository` as "{username}/{repository}".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.username, self.repository)
    }
}

impl RawPluginInner {
    /// Normalize a `RawPluginInner` into a `Plugin` which is simpler and easier
    /// to handle.
    fn normalize(self, name: String, templates: &IndexMap<String, Template>) -> Result<Plugin> {
        let source = if let Some(directory) = self.local {
            Source::Local { directory }
        } else if let Some(url) = self.remote {
            Source::Remote { url }
        } else {
            let url = if let Some(url) = self.git {
                url
            } else if let Some(repository) = self.gist {
                Url::parse(&format!("https://{}/{}", GIST_HOST, repository))
                    .chain(s!("failed to construct Gist URL using `{}`", repository))?
            } else if let Some(repository) = self.github {
                Url::parse(&format!(
                    "https://{}/{}/{}",
                    GITHUB_HOST, repository.username, repository.repository
                ))
                .chain(s!("failed to construct GitHub URL using `{}`", repository))?
            } else {
                // This assumes `deserialize_raw_plugin_inner()` validated correctly.
                unreachable!()
            };

            Source::Git {
                url,
                reference: self.reference,
            }
        };

        // Check whether the specifed templates actually exist.
        if let Some(apply) = &self.apply {
            for name in apply {
                if !templates.contains_key(name) {
                    bail!("unknown template `{}`", name);
                }
            }
        }

        Ok(Plugin {
            name,
            source,
            directory: self.directory,
            uses: self.uses,
            apply: self.apply,
        })
    }
}

impl RawConfig {
    /// Read a `RawConfig` from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let contents = fs::read(&path).chain(s!("failed to read from `{}`", path.display()))?;
        let config: RawConfig = toml::from_str(&String::from_utf8_lossy(&contents))
            .chain(s!("failed to deserialize contents as TOML"))?;
        Ok(config)
    }

    /// Normalize a `RawConfig` into a `Config`.
    pub fn normalize(self) -> Result<Config> {
        let Self {
            matches,
            apply,
            mut templates,
            plugins,
        } = self;

        // Check that the templates can be compiled.
        {
            let mut handlebars = handlebars::Handlebars::new();
            handlebars.set_strict_mode(true);
            for (name, template) in &templates {
                handlebars
                    .register_template_string(&name, &template.value)
                    .chain(s!("failed to compile template `{}`", name))?
            }
        }

        // Add the default templates.
        templates
            .entry("PATH".into())
            .or_insert_with(|| "export PATH=\"{{ directory }}:$PATH\"".into());
        templates
            .entry("path".into())
            .or_insert_with(|| "path=( \"{{ directory }}\" $path )".into());
        templates
            .entry("fpath".into())
            .or_insert_with(|| "fpath=( \"{{ directory }}\" $fpath )".into());
        templates
            .entry("source".into())
            .or_insert_with(|| Template::from("source \"{{ filename }}\"").each(true));

        // Check whether the specifed templates actually exist.
        for name in &apply {
            if !templates.contains_key(name) {
                bail!("unknown template `{}`", name);
            }
        }

        // Normalize the plugins.
        let mut normalized_plugins = Vec::with_capacity(plugins.len());

        for (name, RawPlugin { inner }) in plugins {
            normalized_plugins.push(
                inner
                    .normalize(name.clone(), &templates)
                    .chain(s!("failed to normalize plugin `{}`", name))?,
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

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct TemplateTest {
        t: Template,
    }

    #[test]
    fn deserialize_template_as_str() {
        let test: TemplateTest = toml::from_str("t = 'test'").unwrap();
        assert_eq!(test.t.value, String::from("test"));
        assert_eq!(test.t.each, false);
    }

    #[test]
    fn deserialize_template_as_map() {
        let test: TemplateTest = toml::from_str("t = { value = 'test', each = true }").unwrap();
        assert_eq!(test.t.value, String::from("test"));
        assert_eq!(test.t.each, true);
    }

    #[derive(Deserialize)]
    struct TestGitHubRepository {
        g: GitHubRepository,
    }

    #[test]
    fn deserialize_github_repository() {
        let test: TestGitHubRepository = toml::from_str("g = 'rossmacarthur/sheldon'").unwrap();
        assert_eq!(test.g.username, String::from("rossmacarthur"));
        assert_eq!(test.g.repository, String::from("sheldon"));
    }

    #[test]
    #[should_panic]
    fn deserialize_github_repository_two_slashes() {
        toml::from_str::<TestGitHubRepository>("g = 'rossmacarthur/sheldon/test'").unwrap();
    }

    #[test]
    #[should_panic]
    fn deserialize_github_repository_no_slashes() {
        toml::from_str::<TestGitHubRepository>("g = 'noslash'").unwrap();
    }

    #[derive(Deserialize)]
    struct TestGitReference {
        #[serde(flatten)]
        g: GitReference,
    }

    #[test]
    fn deserialize_git_reference_branch() {
        let test: TestGitReference = toml::from_str("branch = 'master'").unwrap();
        assert_eq!(test.g, GitReference::Branch(String::from("master")));
    }

    #[test]
    fn deserialize_git_reference_tag() {
        let test: TestGitReference = toml::from_str("tag = 'v0.5.1'").unwrap();
        assert_eq!(test.g, GitReference::Tag(String::from("v0.5.1")));
    }

    #[test]
    fn deserialize_git_reference_revision() {
        let test: TestGitReference = toml::from_str("revision = 'cd65e828'").unwrap();
        assert_eq!(test.g, GitReference::Revision(String::from("cd65e828")));
    }

    #[test]
    fn deserialize_raw_plugin_git() {
        let mut expected = RawPlugin::default();
        expected.inner.git = Some(Url::parse("https://github.com/rossmacarthur/sheldon").unwrap());
        let plugin: RawPlugin =
            toml::from_str("git = 'https://github.com/rossmacarthur/sheldon'").unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn deserialize_raw_plugin_github() {
        let mut expected = RawPlugin::default();
        expected.inner.github = Some(GitHubRepository {
            username: "rossmacarthur".into(),
            repository: "sheldon".into(),
        });
        let plugin: RawPlugin = toml::from_str("github = 'rossmacarthur/sheldon'").unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn deserialize_raw_plugin_conflicts() {
        let sources = [
            ("git", "https://github.com/rossmacarthur/sheldon"),
            ("gist", "579d02802b1cc17baed07753d09f5009"),
            ("github", "rossmacarthur/sheldon"),
            ("remote", "https://ross.macarthur.io"),
            ("local", "~/.dotfiles/zsh/pure"),
        ];

        for (a, example_a) in &sources {
            for (b, example_b) in &sources {
                if a == b {
                    continue;
                }
                let text = format!("{} = '{}'\n{} = '{}'", a, example_a, b, example_b);
                let e = toml::from_str::<RawPlugin>(&text).unwrap_err();
                assert_eq!(e.to_string(), "multiple source fields specified",)
            }
        }
    }
}
