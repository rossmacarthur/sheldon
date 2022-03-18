use std::path::{Path, PathBuf};

use anyhow::{bail, Context as ResultExt, Result};
use indexmap::IndexMap;
use maplit::hashmap;

use crate::config::{ExternalPlugin, Source, Template};
use crate::context::Context;
use crate::lock::file::LockedExternalPlugin;
use crate::lock::source::LockedSource;

/// Consume the [`ExternalPlugin`] and convert it to a [`LockedExternalPlugin`].
pub fn lock(
    ctx: &Context,
    templates: &IndexMap<String, Template>,
    locked_source: LockedSource,
    global_matches: &[String],
    global_apply: &[String],
    plugin: ExternalPlugin,
) -> Result<LockedExternalPlugin> {
    let ExternalPlugin {
        name,
        source,
        dir,
        uses,
        apply,
    } = plugin;

    let apply = apply.unwrap_or_else(|| global_apply.to_vec());

    Ok(if let Source::Remote { .. } = source {
        let LockedSource { dir, file } = locked_source;
        LockedExternalPlugin {
            name,
            source_dir: dir,
            plugin_dir: None,
            files: vec![file.unwrap()],
            apply,
        }
    } else {
        // Handlebars instance to do the rendering
        let mut hbs = handlebars::Handlebars::new();
        hbs.set_strict_mode(true);

        // Data to use in template rendering
        let mut data = hashmap! {
            "data_dir" => ctx
                .data_dir()
                .to_str()
                .context("data directory is not valid UTF-8")?,
            "name" => &name
        };

        let source_dir = locked_source.dir;
        let plugin_dir = if let Some(dir) = dir {
            let rendered = hbs
                .render_template(&dir, &data)
                .with_context(s!("failed to render template `{}`", dir))?;
            Some(source_dir.join(rendered))
        } else {
            None
        };
        let dir = plugin_dir.as_ref().unwrap_or(&source_dir);
        let dir_as_str = dir
            .to_str()
            .context("plugin directory is not valid UTF-8")?;
        data.insert("dir", dir_as_str);

        let mut files = Vec::new();

        // If the plugin defined what files to use, we do all of them.
        if let Some(uses) = &uses {
            let patterns = uses
                .iter()
                .map(|u| {
                    hbs.render_template(u, &data)
                        .with_context(s!("failed to render template `{}`", u))
                })
                .collect::<Result<Vec<_>>>()?;
            if !match_globs(dir, &patterns, &mut files)? {
                bail!("failed to find any files matching any of `{:?}`", patterns);
            }
        // Otherwise we try to figure out which files to use...
        } else {
            for g in global_matches {
                let pattern = hbs
                    .render_template(g, &data)
                    .with_context(s!("failed to render template `{}`", g))?;
                if match_globs(dir, &[pattern], &mut files)? {
                    break;
                }
            }
            if files.is_empty()
                && templates
                    .iter()
                    .any(|(key, value)| apply.contains(key) && value.each)
            {
                bail!("no files matched for `{}`", &name);
            }
        }

        LockedExternalPlugin {
            name,
            source_dir,
            plugin_dir,
            files,
            apply,
        }
    })
}

fn match_globs(dir: &Path, patterns: &[String], files: &mut Vec<PathBuf>) -> Result<bool> {
    let debug = || {
        patterns
            .iter()
            .map(|p| format!("`{}`", p))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut matched = false;
    for entry in globwalk::GlobWalkerBuilder::from_patterns(dir, patterns)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .build()
        .with_context(s!("failed to parse glob patterns: {}", debug()))?
    {
        let entry = entry.with_context(s!("failed to match patterns: {}", debug()))?;
        if entry.metadata()?.file_type().is_symlink() {
            entry
                .path()
                .metadata()
                .with_context(s!("failed to read symlink `{}`", entry.path().display()))
                .with_context(s!("failed to match patterns: {}", debug()))?;
        }
        files.push(entry.into_path());
        matched = true;
    }
    Ok(matched)
}

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use url::Url;

    use crate::config::{GitReference, Shell};
    use crate::lock::source;

    #[test]
    fn external_plugin_lock_git_with_uses() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.1.0".to_string())),
            },
            dir: None,
            uses: Some(vec!["*.md".into(), "{{ name }}.plugin.zsh".into()]),
            apply: None,
        };
        let locked_source = source::lock(&ctx, plugin.source.clone()).unwrap();
        let clone_dir = dir.join("repos/github.com/rossmacarthur/sheldon-test");

        let locked = lock(
            &ctx,
            &Shell::default().default_templates().clone(),
            locked_source,
            &[],
            &["hello".into()],
            plugin,
        )
        .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.dir(), clone_dir);
        assert_eq!(
            locked.files,
            vec![
                clone_dir.join("README.md"),
                clone_dir.join("test.plugin.zsh")
            ]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn external_plugin_lock_git_with_matches() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.1.0".to_string())),
            },
            dir: None,
            uses: None,
            apply: None,
        };
        let locked_source = source::lock(&ctx, plugin.source.clone()).unwrap();
        let clone_dir = dir.join("repos/github.com/rossmacarthur/sheldon-test");

        let locked = lock(
            &ctx,
            &Shell::default().default_templates().clone(),
            locked_source,
            &["*.plugin.zsh".to_string()],
            &["hello".to_string()],
            plugin,
        )
        .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.dir(), clone_dir);
        assert_eq!(locked.files, vec![clone_dir.join("test.plugin.zsh")]);
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn external_plugin_lock_git_with_matches_error() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.1.0".to_string())),
            },
            dir: None,
            uses: None,
            apply: None,
        };
        let locked_source = source::lock(&ctx, plugin.source.clone()).unwrap();

        lock(
            &ctx,
            &Shell::default().default_templates().clone(),
            locked_source,
            &["*doesnotexist*".to_string()],
            &["source".to_string()],
            plugin,
        )
        .unwrap_err();
    }

    #[test]
    fn external_plugin_lock_git_with_matches_not_each() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.1.0".to_string())),
            },
            dir: None,
            uses: None,
            apply: None,
        };
        let locked_source = source::lock(&ctx, plugin.source.clone()).unwrap();
        let clone_dir = dir.join("repos/github.com/rossmacarthur/sheldon-test");

        let locked = lock(
            &ctx,
            &Shell::default().default_templates().clone(),
            locked_source,
            &["*doesnotexist*".to_string()],
            &["PATH".to_string()],
            plugin,
        )
        .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.dir(), clone_dir);
        assert!(locked.files.is_empty());
        assert_eq!(locked.apply, vec![String::from("PATH")]);
    }

    #[test]
    fn external_plugin_lock_remote() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = Context::testing(dir);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Remote {
                url: Url::parse(
                    "https://github.com/rossmacarthur/sheldon-test/raw/master/test.plugin.zsh",
                )
                .unwrap(),
            },
            dir: None,
            uses: None,
            apply: None,
        };
        let locked_source = source::lock(&ctx, plugin.source.clone()).unwrap();
        let download_dir = dir.join("downloads/github.com/rossmacarthur/sheldon-test/raw/master");

        let locked = lock(
            &ctx,
            &Shell::default().default_templates().clone(),
            locked_source,
            &[],
            &["hello".to_string()],
            plugin,
        )
        .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.dir(), download_dir);
        assert_eq!(locked.files, vec![download_dir.join("test.plugin.zsh")]);
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }
}
