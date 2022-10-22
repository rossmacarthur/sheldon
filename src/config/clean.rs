use std::collections::HashSet;
use std::path::Path;
use std::{fs, result};

use anyhow::{Context as ResultExt, Error, Result};
use walkdir::WalkDir;

use crate::config::{Config, Plugin, Source};
use crate::context::Context;
use crate::lock::source;

/// Clean the clone and download directories.
pub fn clean(ctx: &Context, warnings: &mut Vec<Error>, config: &Config) -> Result<()> {
    let mut source_dirs = HashSet::new();
    let mut parent_dirs = HashSet::new();
    let mut files = HashSet::new();

    for plugin in &config.plugins {
        if let Plugin::External(plugin) = plugin {
            match &plugin.source {
                Source::Git { url, .. } => {
                    let dir = source::git_dir(ctx, url)?;
                    parent_dirs.extend(dir.ancestors().map(Path::to_path_buf));
                    source_dirs.insert(dir);
                }
                Source::Remote { url } => {
                    let (dir, file) = source::remote_dir_and_file(ctx, url)?;
                    files.insert(file);
                    parent_dirs.extend(dir.ancestors().map(Path::to_path_buf));
                }
                Source::Local { .. } => {
                    // Don't remove local plugins!
                }
            }
        }
    }

    parent_dirs.insert(ctx.clone_dir().to_path_buf());
    parent_dirs.insert(ctx.download_dir().to_path_buf());

    for entry in WalkDir::new(ctx.clone_dir())
        .into_iter()
        .filter_entry(|e| !source_dirs.contains(e.path()))
        .filter_map(result::Result::ok)
        .filter(|e| !parent_dirs.contains(e.path()))
    {
        if let Err(err) = remove_path(ctx, entry.path()) {
            warnings.push(err);
        }
    }

    for entry in WalkDir::new(ctx.download_dir())
        .into_iter()
        .filter_map(result::Result::ok)
        .filter(|e| {
            let p = e.path();
            !files.contains(p) && !parent_dirs.contains(p)
        })
    {
        if let Err(err) = remove_path(ctx, entry.path()) {
            warnings.push(err);
        }
    }

    Ok(())
}

fn remove_path(ctx: &Context, path: &Path) -> Result<()> {
    let path_replace_home = ctx.replace_home(path);
    let path_display = &path_replace_home.display();
    if path
        .metadata()
        .with_context(s!("failed to fetch metadata for `{}`", path_display))?
        .is_dir()
    {
        fs::remove_dir_all(path)
            .with_context(s!("failed to remove directory `{}`", path_display))?;
    } else {
        fs::remove_file(path).with_context(s!("failed to remove file `{}`", path_display))?;
    }
    warning_v!(ctx, "Removed", path_display);
    Ok(())
}
