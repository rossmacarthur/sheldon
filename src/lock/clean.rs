use std::collections::HashSet;
use std::path::Path;
use std::{fs, result};

use anyhow::{Context as ResultExt, Error, Result};
use walkdir::WalkDir;

use crate::context::Context;
use crate::lock::file::LockedPlugin;
use crate::lock::LockedConfig;

impl LockedConfig {
    /// Clean the clone and download directories.
    pub fn clean(&self, ctx: &Context, warnings: &mut Vec<Error>) {
        // Track the source directories, all the plugin directory parents, and all the
        // plugin files.
        let mut source_dirs = HashSet::new();
        let mut parent_dirs = HashSet::new();
        let mut files = HashSet::new();
        for plugin in &self.plugins {
            if let LockedPlugin::External(locked) = plugin {
                source_dirs.insert(locked.source_dir.as_path());
                parent_dirs.extend(locked.dir().ancestors());
                files.extend(locked.files.iter().filter_map(|f| {
                    // `files` is only used when filtering the download directory
                    if f.starts_with(self.ctx.download_dir()) {
                        Some(f.as_path())
                    } else {
                        None
                    }
                }));
            }
        }
        parent_dirs.insert(self.ctx.clone_dir());
        parent_dirs.insert(self.ctx.download_dir());

        for entry in WalkDir::new(self.ctx.clone_dir())
            .into_iter()
            .filter_entry(|e| !source_dirs.contains(e.path()))
            .filter_map(result::Result::ok)
            .filter(|e| !parent_dirs.contains(e.path()))
        {
            if let Err(err) = remove_path(ctx, entry.path()) {
                warnings.push(err);
            }
        }

        for entry in WalkDir::new(self.ctx.download_dir())
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
    }
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
