//! Edit the configuration file.

use std::{fs, path::Path};

use anyhow::{bail, Context as ResultExt, Result};

use crate::config::RawPlugin;

/// An editable plugin.
#[derive(Debug)]
pub struct Plugin {
    inner: RawPlugin,
}

/// An editable config.
#[derive(Debug, Default)]
pub struct Config {
    doc: toml_edit::Document,
}

impl From<RawPlugin> for Plugin {
    fn from(raw_plugin: RawPlugin) -> Self {
        Self { inner: raw_plugin }
    }
}

impl Config {
    /// Read a `Config` from the given string.
    pub fn from_string<S>(s: S) -> Result<Self>
    where
        S: Into<String>,
    {
        let doc = s
            .into()
            .parse::<toml_edit::Document>()
            .context("failed to deserialize contents as TOML")?;
        Ok(Self { doc })
    }

    /// Read a `Config` from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let contents = String::from_utf8(
            fs::read(&path).with_context(s!("failed to read from `{}`", path.display()))?,
        )
        .context("config file contents are not valid UTF-8")?;
        Self::from_string(contents)
    }

    /// Add a new plugin.
    pub fn add(&mut self, name: &str, plugin: Plugin) -> Result<()> {
        let content =
            toml::to_string_pretty(&plugin.inner).expect("failed to serialize plugin as TOML");

        let mini = content
            .parse::<toml_edit::Document>()
            .expect("failed to parse valid TOML");

        match self.doc.as_table_mut().entry("plugins") {
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

    /// Write a `Config` to the given path.
    pub fn to_path<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        fs::write(path, self.doc.to_string())
            .with_context(s!("failed to write config to `{}`", path.display()))
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GitReference;
    use std::{io::Write, path::PathBuf};
    use url::Url;

    #[test]
    fn config_from_string_invalid() {
        Config::from_string("x = \n").unwrap_err();
    }

    #[test]
    fn config_from_path() {
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
        Config::from_path(path).unwrap();
    }

    #[test]
    fn config_from_path_example() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("docs/plugins.example.toml");
        Config::from_path(path).unwrap();
    }

    #[test]
    fn config_empty_add_git() {
        let mut config = Config::from_string("").unwrap();
        config
            .add(
                "sheldon-test",
                Plugin::from(RawPlugin {
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
    fn config_empty_add_github() {
        let mut config = Config::from_string("").unwrap();
        config
            .add(
                "sheldon-test",
                Plugin::from(RawPlugin {
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
    fn config_others_add_git() {
        let mut config = Config::from_string(
            r#"
# test configuration file
apply = ["PATH", "source"]

[templates]
prompt = { value = 'ln -sf "{{ file }}" "{{ root }}/functions/prompt_{{ name }}_setup"', each = true }

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
                Plugin::from(RawPlugin {
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
prompt = { value = 'ln -sf "{{ file }}" "{{ root }}/functions/prompt_{{ name }}_setup"', each = true }

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
