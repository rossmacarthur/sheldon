//! The config file.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
    result, str,
};

use indexmap::IndexMap;
use serde::{self, de, Deserialize, Deserializer};
use url::Url;

use crate::{
    config::{Config, ExternalPlugin, GitReference, InlinePlugin, Plugin, Source, Template},
    Result, ResultExt,
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
struct RawPlugin {
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
    /// An inline script.
    inline: Option<String>,
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

/// The contents of the configuration file.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct RawConfig {
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    matches: Vec<String>,
    /// The default list of template names to apply to each matched file.
    apply: Option<Vec<String>>,
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

impl Default for RawConfig {
    /// Returns the default `RawConfig`.
    fn default() -> Self {
        Self {
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
            apply: None,
            templates: IndexMap::new(),
            plugins: IndexMap::new(),
        }
    }
}

/////////////////////////////////////////////////////////////////////////
// Normalization implementations
/////////////////////////////////////////////////////////////////////////

// Check whether the specifed templates actually exist.
fn validate_template_names(
    apply: &Option<Vec<String>>,
    templates: &IndexMap<String, Template>,
) -> Result<()> {
    if let Some(apply) = apply {
        for name in apply {
            if !crate::lock::DEFAULT_TEMPLATES.contains_key(name) && !templates.contains_key(name) {
                bail!("unknown template `{}`", name);
            }
        }
    }
    Ok(())
}

impl fmt::Display for GitHubRepository {
    /// Displays a `GitHubRepository` as "{username}/{repository}".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.username, self.repository)
    }
}

impl Source {
    fn is_git(&self) -> bool {
        match *self {
            Self::Git { .. } => true,
            _ => false,
        }
    }
}

/// A convenience struct to help with normalizing the config file in
/// `Rawplugin::normalize()`.
#[derive(Debug)]
enum TempSource {
    External(Source),
    Inline(String),
}

impl RawPlugin {
    /// Normalize a `RawPlugin` into a `Plugin` which is simpler and easier
    /// to handle.
    fn normalize(self, name: String, templates: &IndexMap<String, Template>) -> Result<Plugin> {
        let Self {
            git,
            gist,
            github,
            remote,
            local,
            inline,
            reference,
            directory,
            uses,
            apply,
        } = self;

        let reference_is_some = reference.is_some();

        let raw_source = match (git, gist, github, remote, local, inline) {
            // `git` type
            (Some(url), None, None, None, None, None) => {
                TempSource::External(Source::Git { url, reference })
            }
            // `gist` type
            (None, Some(identifier), None, None, None, None) => {
                let url = Url::parse(&format!("https://{}/{}", GIST_HOST, identifier))
                    .chain(s!("failed to construct Gist URL using `{}`", identifier))?;
                TempSource::External(Source::Git { url, reference })
            }
            // `github` type
            (None, None, Some(repository), None, None, None) => {
                let url = Url::parse(&format!(
                    "https://{}/{}/{}",
                    GITHUB_HOST, repository.username, repository.repository
                ))
                .chain(s!("failed to construct GitHub URL using `{}`", repository))?;
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
                if !source.is_git() && reference_is_some {
                    bail!(
                        "`branch`, `tag`, and `revision` fields are not supported by this plugin \
                         type"
                    );
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
                for (field, is_some) in [
                    (
                        "`branch`, `tag`, and `revision` fields are",
                        reference_is_some,
                    ),
                    ("`directory` field is", directory.is_some()),
                    ("`use` field is", uses.is_some()),
                    ("`apply` field is", apply.is_some()),
                ]
                .iter()
                {
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
        let contents = fs::read(&path).chain(s!("failed to read from `{}`", path.display()))?;
        let config: Self = toml::from_str(&String::from_utf8_lossy(&contents))
            .chain(s!("failed to deserialize contents as TOML"))?;
        Ok(config)
    }

    /// Normalize a `RawConfig` into a `Config`.
    pub fn normalize(self) -> Result<Config> {
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
                    .chain(s!("failed to compile template `{}`", name))?
            }
        }

        validate_template_names(&apply, &templates)?;

        // Normalize the plugins.
        let mut normalized_plugins = Vec::with_capacity(plugins.len());

        for (name, plugin) in plugins {
            normalized_plugins.push(
                plugin
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
        expected.git = Some(Url::parse("https://github.com/rossmacarthur/sheldon").unwrap());
        let plugin: RawPlugin =
            toml::from_str("git = 'https://github.com/rossmacarthur/sheldon'").unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn deserialize_raw_plugin_github() {
        let mut expected = RawPlugin::default();
        expected.github = Some(GitHubRepository {
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
}
