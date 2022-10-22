//! Edit the configuration file.

use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context as ResultExt, Result};

use crate::config::{RawPlugin, Shell};

/// An editable plugin.
#[derive(Debug)]
pub struct EditPlugin {
    inner: RawPlugin,
}

/// An editable config.
#[derive(Debug)]
pub struct EditConfig {
    /// The parsed TOML version of the config.
    doc: toml_edit::Document,
}

impl From<RawPlugin> for EditPlugin {
    fn from(raw_plugin: RawPlugin) -> Self {
        Self { inner: raw_plugin }
    }
}

impl fmt::Display for EditConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.doc)
    }
}

impl EditConfig {
    /// Returns the default config for the given shell.
    pub fn default(shell: Option<Shell>) -> Self {
        let config = include_str!("plugins.toml");
        match shell {
            Some(shell) => {
                // FIXME: figure out how to do this with `toml_edit`, it always places
                //        shell = ... above the header comment.
                let config = config.replace(
                    "\n[plugins]",
                    &format!("\nshell = \"{}\"\n\n[plugins]", shell),
                );
                Self::from_str(config).unwrap()
            }
            None => Self::from_str(config).unwrap(),
        }
    }

    /// Read the config from the given string.
    pub fn from_str<S>(s: S) -> Result<Self>
    where
        S: AsRef<str>,
    {
        let doc = s
            .as_ref()
            .parse::<toml_edit::Document>()
            .context("failed to deserialize contents as TOML")?;
        Ok(Self { doc })
    }

    /// Read the config from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read from `{}`", path.display()))?;
        Self::from_str(contents)
    }

    /// Add a new plugin.
    pub fn add(&mut self, name: &str, plugin: &EditPlugin) -> Result<()> {
        let contents =
            toml::to_string_pretty(&plugin.inner).expect("failed to serialize plugin as TOML");

        let mini = contents
            .parse::<toml_edit::Document>()
            .expect("failed to parse valid TOML");

        match &mut self.doc["plugins"] {
            item @ toml_edit::Item::None => {
                let mut plugins = toml_edit::Table::new();
                plugins.set_implicit(true);
                *item = toml_edit::Item::Table(plugins);
            }
            toml_edit::Item::Table(_) => {}
            _ => bail!("current `plugins` entry is not a table"),
        }

        match &mut self.doc["plugins"][name] {
            item @ toml_edit::Item::None => {
                let mut table = toml_edit::table();
                for (k, v) in mini.as_table().iter() {
                    table[k] = v.clone();
                }
                *item = table;
            }
            _ => bail!("plugin with name `{}` already exists", name),
        }

        Ok(())
    }

    /// Remove a plugin.
    pub fn remove(&mut self, name: &str) {
        self.doc["plugins"][name] = toml_edit::Item::None;
    }

    /// Write a config to the given path.
    pub fn to_path<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        fs::write(path, self.to_string())
            .with_context(|| format!("failed to write config to `{}`", path.display()))
    }
}

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use pretty_assertions::assert_eq;
    use url::Url;

    use crate::config::GitReference;

    #[test]
    fn edit_config_default() {
        EditConfig::default(None);
    }

    #[test]
    fn edit_config_default_bash() {
        EditConfig::default(Some(Shell::Bash));
    }

    #[test]
    fn edit_config_default_zsh() {
        EditConfig::default(Some(Shell::Zsh));
    }

    #[test]
    fn edit_config_from_str_invalid() {
        EditConfig::from_str("x = \n").unwrap_err();
    }

    #[test]
    fn edit_config_from_path() {
        let mut temp = tempfile::NamedTempFile::new().unwrap();
        write!(
            temp,
            r#"
# test configuration file

[plugins.test]
github = "rossmacarthur/sheldon-test"
tag = "0.1.0"
        "#
        )
        .unwrap();
        let path = temp.into_temp_path();
        EditConfig::from_path(path).unwrap();
    }

    #[test]
    fn edit_config_empty_add_git() {
        let mut config = EditConfig::from_str("").unwrap();
        config
            .add(
                "sheldon-test",
                &EditPlugin::from(RawPlugin {
                    git: Some(Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap()),
                    reference: Some(GitReference::Branch("feature".to_string())),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(
            config.doc.to_string(),
            r#"
[plugins.sheldon-test]
git = 'https://github.com/rossmacarthur/sheldon-test'
branch = 'feature'
"#
        )
    }

    #[test]
    fn edit_config_empty_add_github() {
        let mut config = EditConfig::from_str("").unwrap();
        config
            .add(
                "sheldon-test",
                &EditPlugin::from(RawPlugin {
                    github: Some("rossmacarthur/sheldon-test".parse().unwrap()),
                    reference: Some(GitReference::Tag("0.1.0".to_string())),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(
            config.doc.to_string(),
            r#"
[plugins.sheldon-test]
github = 'rossmacarthur/sheldon-test'
tag = '0.1.0'
"#
        )
    }

    #[test]
    fn edit_config_others_add_git() {
        let mut config = EditConfig::from_str(
            r#"
# test configuration file
apply = ["PATH", "source"]

[templates]
prompt = '{% for file in files %}ln -sf "{{ file }}" "{{ data_dir }}/functions/prompt_{{ name }}_setup"{% endfor %}'

# yes this is the pure plugin
[plugins.pure]
github = "sindresorhus/pure"
apply = ["prompt"]
use = ["{{ name }}.zsh"]
    "#,
        )
        .unwrap();
        config
            .add(
                "sheldon-test",
                &EditPlugin::from(RawPlugin {
                    github: Some("rossmacarthur/sheldon-test".parse().unwrap()),
                    reference: Some(GitReference::Tag("0.1.0".to_string())),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(
            config.doc.to_string(),
            r#"
# test configuration file
apply = ["PATH", "source"]

[templates]
prompt = '{% for file in files %}ln -sf "{{ file }}" "{{ data_dir }}/functions/prompt_{{ name }}_setup"{% endfor %}'

# yes this is the pure plugin
[plugins.pure]
github = "sindresorhus/pure"
apply = ["prompt"]
use = ["{{ name }}.zsh"]

[plugins.sheldon-test]
github = 'rossmacarthur/sheldon-test'
tag = '0.1.0'
    "#
        )
    }
}
