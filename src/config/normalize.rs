//! Normalize a raw config from the file into a [`Config`].

use std::str;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context as ResultExt, Error, Result};
use indexmap::IndexMap;
use url::Url;

use crate::config::file::{GitProtocol, RawConfig, RawPlugin};
use crate::config::{Config, ExternalPlugin, InlinePlugin, Plugin, Shell, Source};
use crate::util::TEMPLATE_ENGINE;

/// The Gist domain host.
const GIST_HOST: &str = "gist.github.com";

/// The GitHub domain host.
const GITHUB_HOST: &str = "github.com";

/// Normalize a raw config from the file into a [`Config`].
pub fn normalize(raw_config: RawConfig, warnings: &mut Vec<Error>) -> Result<Config> {
    let RawConfig {
        shell,
        matches,
        apply,
        templates,
        plugins,
        rest,
    } = raw_config;

    check_extra_toml(rest, |key| {
        warnings.push(anyhow!("unused config key: `{key}`"))
    });

    // Check that the templates can be compiled.
    for (name, template) in &templates {
        TEMPLATE_ENGINE
            .compile(template)
            .with_context(|| format!("failed to compile template `{name}`"))?;
    }

    let shell = shell.unwrap_or_default();

    validate_template_names(shell, &apply, &templates)?;

    // Normalize the plugins.
    let mut normalized_plugins = Vec::with_capacity(plugins.len());

    for (name, plugin) in plugins {
        normalized_plugins.push(
            normalize_plugin(plugin, name.clone(), shell, &templates, warnings)
                .with_context(|| format!("failed to normalize plugin `{name}`"))?,
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

/// Normalize a raw plugin from the file into a [`Plugin`] which is simpler and
/// easier to handle.
///
/// For example gist and github sources are converted to a [`Source::Git`].
fn normalize_plugin(
    raw_plugin: RawPlugin,
    name: String,
    shell: Shell,
    templates: &IndexMap<String, String>,
    warnings: &mut Vec<Error>,
) -> Result<Plugin> {
    enum TempSource {
        External(Source),
        Inline(String),
    }

    let RawPlugin {
        git,
        gist,
        github,
        remote,
        local,
        inline,
        mut proto,
        reference,
        dir,
        uses,
        apply,
        profiles,
        mut rest,
    } = raw_plugin;

    let is_reference_some = reference.is_some();
    let is_gist_or_github = gist.is_some() || github.is_some();

    // Handle some deprecated items :/
    if proto.is_none() {
        if let Some(protocol) = try_pop_toml_value(&mut rest, "protocol") {
            warnings.push(anyhow!(
                "use of deprecated config key: `plugins.{name}.protocol`, please use \
                 `plugins.{name}.proto` instead",
                name = name,
            ));
            proto = Some(protocol);
        }
    }

    check_extra_toml(rest, |key| {
        warnings.push(anyhow!("unused config key: `plugins.{name}.{key}`"))
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
                repository
            );
            let url = Url::parse(&url_str)
                .with_context(|| format!("failed to construct Gist URL using `{repository}`"))?;
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
                .with_context(|| format!("failed to construct GitHub URL using `{repository}`"))?;
            TempSource::External(Source::Git { url, reference })
        }
        // `remote` type
        (None, None, None, Some(url), None, None) => TempSource::External(Source::Remote { url }),
        // `local` type
        (None, None, None, None, Some(dir), None) => TempSource::External(Source::Local { dir }),
        // `inline` type
        (None, None, None, None, None, Some(raw)) => TempSource::Inline(raw),
        (None, None, None, None, None, None) => {
            bail!("plugin `{name}` has no source fields");
        }
        _ => {
            bail!("plugin `{name}` has multiple source fields");
        }
    };

    match raw_source {
        TempSource::External(source) => {
            if !source.is_git() && is_reference_some {
                bail!(
                    "the `branch`, `tag`, and `rev` fields are not supported by this plugin type"
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
                profiles,
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
                    bail!("the {field} not supported by inline plugins");
                }
            }
            Ok(Plugin::Inline(InlinePlugin {
                name,
                raw,
                profiles,
            }))
        }
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
    /// Whether this is a Git source.
    fn is_git(&self) -> bool {
        matches!(*self, Self::Git { .. })
    }
}

/// Try and pop the TOML value from the table.
fn try_pop_toml_value<T>(rest: &mut Option<toml::Value>, key: &str) -> Option<T>
where
    T: FromStr,
{
    if let Some(toml::Value::Table(table)) = rest {
        if let Some(toml::Value::String(s)) = table.get(key) {
            let result = s.parse().ok();
            if result.is_some() {
                table.remove(key);
            }
            return result;
        }
    }
    None
}

/// Call the given function on all extra TOML keys.
fn check_extra_toml<F>(rest: Option<toml::Value>, mut f: F)
where
    F: FnMut(&str),
{
    if let Some(toml::Value::Table(table)) = rest {
        for key in table.keys() {
            f(key)
        }
    }
}

/// Check whether the specifed templates actually exist.
fn validate_template_names(
    shell: Shell,
    apply: &Option<Vec<String>>,
    templates: &IndexMap<String, String>,
) -> Result<()> {
    if let Some(apply) = apply {
        for name in apply {
            if !shell.default_templates().contains_key(name) && !templates.contains_key(name) {
                bail!("unknown template `{name}`");
            }
        }
    }
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::{GitHubRepository, GitReference};

    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_plugin_conflicts() {
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
                let text = format!("{a} = '{example_a}'\n{b} = '{example_b}'");
                let raw = toml::from_str::<RawPlugin>(&text).unwrap();
                let err = normalize_plugin(
                    raw,
                    "test".to_string(),
                    Shell::default(),
                    &IndexMap::new(),
                    &mut Vec::new(),
                )
                .unwrap_err();
                assert_eq!(err.to_string(), "plugin `test` has multiple source fields")
            }
        }
    }

    #[test]
    fn normalize_plugin_git() {
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
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            git: Some(url),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_gist_with_git() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse(
                    "git://gist.github.com/rossmacarthur/579d02802b1cc17baed07753d09f5009",
                )
                .unwrap(),
                reference: None,
            },
            dir: None,
            uses: None,
            apply: None,
            profiles: None,
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
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_gist_with_https() {
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
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            gist: Some("579d02802b1cc17baed07753d09f5009".parse().unwrap()),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_gist_with_ssh() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Git {
                url: Url::parse(
                    "ssh://git@gist.github.com/rossmacarthur/579d02802b1cc17baed07753d09f5009",
                )
                .unwrap(),
                reference: None,
            },
            dir: None,
            uses: None,
            apply: None,
            profiles: None,
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
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_github_with_git() {
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
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            proto: Some(GitProtocol::Git),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_github_with_https() {
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
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_github_with_ssh() {
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
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            proto: Some(GitProtocol::Ssh),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_remote() {
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
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            remote: Some(url),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_remote_with_reference() {
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
        let err = normalize_plugin(
            raw_plugin,
            "test".to_string(),
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "the `branch`, `tag`, and `rev` fields are not supported by this plugin type"
        );
    }

    #[test]
    fn normalize_plugin_remote_with_ssh() {
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
        let err = normalize_plugin(
            raw_plugin,
            "test".to_string(),
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "the `proto` field is not supported by this plugin type"
        );
    }

    #[test]
    fn normalize_plugin_local() {
        let name = "test".to_string();
        let expected = Plugin::External(ExternalPlugin {
            name: name.clone(),
            source: Source::Local {
                dir: "/home/temp".into(),
            },
            dir: None,
            uses: None,
            apply: None,
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            local: Some("/home/temp".into()),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_inline() {
        let name = "test".to_string();
        let expected = Plugin::Inline(InlinePlugin {
            name: name.clone(),
            raw: "echo 'this is a test'\n".to_string(),
            profiles: None,
        });
        let raw_plugin = RawPlugin {
            inline: Some("echo 'this is a test'\n".to_string()),
            ..Default::default()
        };
        let plugin = normalize_plugin(
            raw_plugin,
            name,
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(plugin, expected);
    }

    #[test]
    fn normalize_plugin_inline_apply() {
        let raw_plugin = RawPlugin {
            inline: Some("echo 'this is a test'\n".to_string()),
            apply: Some(vec_into!["test"]),
            ..Default::default()
        };
        let err = normalize_plugin(
            raw_plugin,
            "test".to_string(),
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "the `apply` field is not supported by inline plugins"
        );
    }

    #[test]
    fn normalize_plugin_external_invalid_template() {
        let raw_plugin = RawPlugin {
            github: Some(GitHubRepository {
                owner: "rossmacarthur".to_string(),
                name: "sheldon-test".to_string(),
            }),
            apply: Some(vec_into!["test"]),
            ..Default::default()
        };
        let err = normalize_plugin(
            raw_plugin,
            "test".to_string(),
            Shell::default(),
            &IndexMap::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "unknown template `test`");
    }
}
