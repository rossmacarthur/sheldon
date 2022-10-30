//! The raw config file.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::result;
use std::str;
use std::str::FromStr;

use anyhow::Result;
use indexmap::IndexMap;
use regex_macro::regex;
use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use url::Url;

use crate::config::{GitReference, Shell};

/// The contents of the configuration file.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawConfig {
    /// What type of shell is being used.
    pub shell: Option<Shell>,
    /// Which files to match and use in a plugin's directory.
    #[serde(rename = "match")]
    pub matches: Option<Vec<String>>,
    /// The default list of template names to apply to each matched file.
    pub apply: Option<Vec<String>>,
    /// A map of name to template string.
    pub templates: IndexMap<String, String>,
    /// A map of name to plugin.
    pub plugins: IndexMap<String, RawPlugin>,
    /// Any extra keys,
    #[serde(flatten, deserialize_with = "deserialize_rest_toml_value")]
    pub rest: Option<toml::Value>,
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
    /// If configured, only installs this plugin if one of the given profiles is
    /// set in the SHELDON_PROFILE environment variable.
    pub profiles: Option<Vec<String>>,
    /// Hooks executed during template evaluation.
    pub hooks: Option<BTreeMap<String, String>>,
    /// Any extra keys,
    #[serde(flatten, deserialize_with = "deserialize_rest_toml_value")]
    pub rest: Option<toml::Value>,
}

/// A Gist repository identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GistRepository {
    /// The GitHub username / organization.
    pub owner: Option<String>,
    /// The Gist identifier.
    pub identifier: String,
}

/// A GitHub repository identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepository {
    /// The GitHub username / organization.
    pub owner: String,
    /// The GitHub repository name.
    pub name: String,
}

/// The Git protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitProtocol {
    Git,
    Https,
    Ssh,
}

////////////////////////////////////////////////////////////////////////////////
// Serialization implementations
////////////////////////////////////////////////////////////////////////////////

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bash => f.write_str("bash"),
            Self::Zsh => f.write_str("zsh"),
        }
    }
}

impl fmt::Display for GitProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git => f.write_str("git"),
            Self::Https => f.write_str("https"),
            Self::Ssh => f.write_str("ssh"),
        }
    }
}

impl fmt::Display for GistRepository {
    /// Displays as "[{owner}/]{identifier}".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    /// Displays as "{owner}/{repository}".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl_serialize_as_str! { Shell }
impl_serialize_as_str! { GitProtocol }
impl_serialize_as_str! { GistRepository }
impl_serialize_as_str! { GitHubRepository }

////////////////////////////////////////////////////////////////////////////////
// Deserialization implementations
////////////////////////////////////////////////////////////////////////////////

impl Default for Shell {
    fn default() -> Self {
        Self::Zsh
    }
}

/// Produced when we fail to parse the shell type.
#[derive(Debug, Error)]
#[error("expected one of `bash` or `zsh`, got `{}`", self.0)]
pub struct ParseShellError(String);

impl FromStr for Shell {
    type Err = ParseShellError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match &*s.to_lowercase() {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            s => Err(ParseShellError(s.to_string())),
        }
    }
}

/// Produced when we fail to parse a Git protocol.
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

/// Produced when we fail to parse a Gist repository.
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

/// Produced when we fail to parse a GitHub repository.
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

                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str($expecting)
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

impl_deserialize_from_str! { shell, Shell, "a supported shell type" }
impl_deserialize_from_str! { git_protocol, GitProtocol, "a Git protocol type" }
impl_deserialize_from_str! { gist_repository, GistRepository, "a Gist identifier" }
impl_deserialize_from_str! { github_repository, GitHubRepository, "a GitHub repository" }

/// Deserialize the remaining keys into an [`Option<toml::Value>`]. Empty tables
/// are coerced to [`None`].
fn deserialize_rest_toml_value<'de, D>(deserializer: D) -> Result<Option<toml::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: toml::Value = de::Deserialize::deserialize(deserializer)?;
    Ok(match value {
        toml::Value::Table(table) if table.is_empty() => None,
        value => Some(value),
    })
}

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

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
    fn raw_plugin_deserialize_profiles() {
        let expected = RawPlugin {
            profiles: Some(vec!["p1".into(), "p2".into()]),
            ..Default::default()
        };
        let plugin: RawPlugin = toml::from_str("profiles = ['p1', 'p2']").unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn raw_plugin_deserialize_hooks() {
        let expected = RawPlugin {
            hooks: Some(BTreeMap::from([
                ("pre".into(), "PRE".into()),
                ("post".into(), "POST".into()),
            ])),
            ..Default::default()
        };
        let plugin: RawPlugin = toml::from_str("hooks.pre = 'PRE'\nhooks.post = 'POST'").unwrap();
        assert_eq!(plugin, expected);
    }
}
