//! Plugin installation.
//!
//! This module handles the downloading of `Source`s and figuring out which
//! files to use for `Plugins`.

use std::cmp;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::result;
use std::sync;

use anyhow::{anyhow, bail, Context as ResultExt, Error, Result};
use indexmap::{indexmap, IndexMap};
use itertools::{Either, Itertools};
use maplit::hashmap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use url::Url;
use walkdir::WalkDir;

use crate::config::{
    Config, ExternalPlugin, GitReference, InlinePlugin, Plugin, Shell, Source, Template,
};
use crate::context::{LockContext as Context, LockMode as Mode, Settings, SettingsExt};
use crate::util::git;
use crate::util::{self, TempPath};

/////////////////////////////////////////////////////////////////////////
// Locked configuration definitions
/////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
enum GitCheckout {
    /// Checkout the latest of the default branch (HEAD).
    DefaultBranch,
    /// Checkout the tip of a branch.
    Branch(String),
    /// Checkout a specific revision.
    Rev(String),
    /// Checkout a tag.
    Tag(String),
}

/// A locked `Source`.
#[derive(Clone, Debug)]
struct LockedSource {
    /// The clone or download directory.
    dir: PathBuf,
    /// The downloaded file.
    file: Option<PathBuf>,
}

/// A locked `ExternalPlugin`.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct LockedExternalPlugin {
    /// The name of this plugin.
    name: String,
    /// The directory that this plugin's source resides in.
    source_dir: PathBuf,
    /// The directory that this plugin resides in (inside the source directory).
    plugin_dir: Option<PathBuf>,
    /// The files to use in the plugin directory.
    files: Vec<PathBuf>,
    /// What templates to apply to each file.
    apply: Vec<String>,
}

/// A locked `Plugin`.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
enum LockedPlugin {
    External(LockedExternalPlugin),
    Inline(InlinePlugin),
}

/// A locked `Config`.
#[derive(Debug, Deserialize, Serialize)]
pub struct LockedConfig {
    /// The global context that was used to generated this `LockedConfig`.
    #[serde(flatten)]
    pub settings: Settings,
    /// Each locked plugin.
    plugins: Vec<LockedPlugin>,
    /// A map of name to template.
    ///
    /// Note: this field must come last in the struct for it to serialize
    /// properly.
    templates: IndexMap<String, Template>,
    /// Any errors that occurred while generating this `LockedConfig`.
    #[serde(skip)]
    pub errors: Vec<Error>,
}

/////////////////////////////////////////////////////////////////////////
// Lock implementations.
/////////////////////////////////////////////////////////////////////////

impl Shell {
    /// The default files to match on for this shell.
    pub fn default_matches(&self) -> &Vec<String> {
        static DEFAULT_MATCHES_BASH: Lazy<Vec<String>> = Lazy::new(|| {
            vec_into![
                "{{ name }}.plugin.bash",
                "{{ name }}.plugin.sh",
                "{{ name }}.bash",
                "{{ name }}.sh",
                "*.plugin.bash",
                "*.plugin.sh",
                "*.bash",
                "*.sh"
            ]
        });
        static DEFAULT_MATCHES_ZSH: Lazy<Vec<String>> = Lazy::new(|| {
            vec_into![
                "{{ name }}.plugin.zsh",
                "{{ name }}.zsh",
                "{{ name }}.sh",
                "{{ name }}.zsh-theme",
                "*.plugin.zsh",
                "*.zsh",
                "*.sh",
                "*.zsh-theme"
            ]
        });
        match self {
            Self::Bash => &DEFAULT_MATCHES_BASH,
            Self::Zsh => &DEFAULT_MATCHES_ZSH,
        }
    }

    /// The default templates for this shell.
    pub fn default_templates(&self) -> &IndexMap<String, Template> {
        static DEFAULT_TEMPLATES_BASH: Lazy<IndexMap<String, Template>> = Lazy::new(|| {
            indexmap_into! {
                "PATH" => "export PATH=\"{{ dir }}:$PATH\"",
                "source" => Template::from("source \"{{ file }}\"").each(true)
            }
        });
        static DEFAULT_TEMPLATES_ZSH: Lazy<IndexMap<String, Template>> = Lazy::new(|| {
            indexmap_into! {
                "PATH" => "export PATH=\"{{ dir }}:$PATH\"",
                "path" => "path=( \"{{ dir }}\" $path )",
                "fpath" => "fpath=( \"{{ dir }}\" $fpath )",
                "source" => Template::from("source \"{{ file }}\"").each(true)
            }
        });
        match self {
            Self::Bash => &DEFAULT_TEMPLATES_BASH,
            Self::Zsh => &DEFAULT_TEMPLATES_ZSH,
        }
    }

    /// The default template names to apply.
    pub fn default_apply(&self) -> &Vec<String> {
        static DEFAULT_APPLY: Lazy<Vec<String>> = Lazy::new(|| vec_into!["source"]);
        &DEFAULT_APPLY
    }
}

impl From<Option<GitReference>> for GitCheckout {
    fn from(reference: Option<GitReference>) -> Self {
        match reference {
            None => Self::DefaultBranch,
            Some(GitReference::Branch(s)) => Self::Branch(s),
            Some(GitReference::Rev(s)) => Self::Rev(s),
            Some(GitReference::Tag(s)) => Self::Tag(s),
        }
    }
}

impl fmt::Display for GitCheckout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DefaultBranch => write!(f, ""),
            Self::Branch(s) | Self::Rev(s) | Self::Tag(s) => write!(f, "@{}", s),
        }
    }
}

impl GitCheckout {
    /// Resolve `GitCheckout` to a Git object identifier.
    ///
    /// From Cargo: https://github.com/rust-lang/cargo/blob/b49ccadb/src/cargo/sources/git/utils.rs#L308-L381
    fn resolve(&self, repo: &git2::Repository) -> Result<git2::Oid> {
        match self {
            Self::DefaultBranch => git::resolve_head(repo),
            Self::Branch(s) => git::resolve_branch(repo, s),
            Self::Rev(s) => git::resolve_rev(repo, s),
            Self::Tag(s) => git::resolve_tag(repo, s),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Git { url, reference } => {
                let checkout: GitCheckout = reference.clone().into();
                write!(f, "{}{}", url, checkout)
            }
            Self::Remote { url, .. } => write!(f, "{}", url),
            Self::Local { dir } => write!(f, "{}", dir.display()),
        }
    }
}

impl Source {
    fn lock_git_install(
        ctx: &Context,
        dir: PathBuf,
        url: Url,
        checkout: GitCheckout,
    ) -> Result<LockedSource> {
        let temp_dir = TempPath::new(&dir);
        let repo = git::clone(&url, &temp_dir.path())?;
        git::checkout(&repo, checkout.resolve(&repo)?)?;
        git::submodule_update(&repo).context("failed to recursively update")?;
        temp_dir
            .rename(&dir)
            .context("failed to rename temporary clone directory")?;
        status!(ctx, "Cloned", &format!("{}{}", url, checkout));
        Ok(LockedSource { dir, file: None })
    }

    /// Checks if a repository is correctly checked out, if not checks it out.
    fn lock_git_checkout(
        ctx: &Context,
        repo: &git2::Repository,
        url: &Url,
        checkout: GitCheckout,
    ) -> Result<()> {
        let current_oid = repo.head()?.target().context("current HEAD as no target")?;
        let expected_oid = checkout.resolve(&repo)?;
        if current_oid == expected_oid {
            status!(ctx, "Checked", &format!("{}{}", url, checkout))
        } else {
            git::checkout(&repo, expected_oid)?;
            git::submodule_update(&repo).context("failed to recursively update")?;
            status!(
                ctx,
                "Updated",
                &format!(
                    "{}{} ({} to {})",
                    url,
                    checkout,
                    &current_oid.to_string()[..7],
                    &expected_oid.to_string()[..7]
                )
            );
        }
        Ok(())
    }

    /// Clones a Git repository and checks it out at a particular revision.
    fn lock_git(
        ctx: &Context,
        dir: PathBuf,
        url: Url,
        checkout: GitCheckout,
    ) -> Result<LockedSource> {
        match ctx.mode {
            Mode::Normal => match git::open(&dir) {
                Ok(repo) => {
                    if Self::lock_git_checkout(ctx, &repo, &url, checkout.clone()).is_err() {
                        git::fetch(&repo)?;
                        Self::lock_git_checkout(ctx, &repo, &url, checkout)?;
                    }
                    Ok(LockedSource { dir, file: None })
                }
                Err(_) => Self::lock_git_install(ctx, dir, url, checkout),
            },
            Mode::Update => match git::open(&dir) {
                Ok(repo) => {
                    git::fetch(&repo)?;
                    Self::lock_git_checkout(ctx, &repo, &url, checkout)?;
                    Ok(LockedSource { dir, file: None })
                }
                Err(_) => Self::lock_git_install(ctx, dir, url, checkout),
            },
            Mode::Reinstall => Self::lock_git_install(ctx, dir, url, checkout),
        }
    }

    /// Downloads a Remote source.
    fn lock_remote(ctx: &Context, dir: PathBuf, file: PathBuf, url: Url) -> Result<LockedSource> {
        if matches!(ctx.mode, Mode::Normal) && file.exists() {
            status!(ctx, "Checked", &url);
            return Ok(LockedSource {
                dir,
                file: Some(file),
            });
        }

        let mut response =
            util::download(url.clone()).with_context(s!("failed to download `{}`", url))?;
        fs::create_dir_all(&dir).with_context(s!("failed to create dir `{}`", dir.display()))?;
        let mut temp_file = TempPath::new(&file);
        temp_file.write(&mut response).with_context(s!(
            "failed to copy contents to `{}`",
            temp_file.path().display()
        ))?;
        temp_file
            .rename(&file)
            .context("failed to rename temporary download file")?;
        status!(ctx, "Fetched", &url);

        Ok(LockedSource {
            dir,
            file: Some(file),
        })
    }

    /// Checks that a Local source directory exists.
    fn lock_local(ctx: &Context, dir: PathBuf) -> Result<LockedSource> {
        let dir = ctx.expand_tilde(dir);

        if dir.exists() && dir.is_dir() {
            status!(ctx, "Checked", dir.as_path());
            Ok(LockedSource { dir, file: None })
        } else if let Ok(walker) = globwalk::glob(dir.to_string_lossy()) {
            let mut directories: Vec<_> = walker
                .filter_map(|result| match result {
                    Ok(entry) if entry.path().is_dir() => Some(entry.into_path()),
                    _ => None,
                })
                .collect();

            if directories.len() == 1 {
                let dir = directories.remove(0);
                status!(ctx, "Checked", dir.as_path());
                Ok(LockedSource { dir, file: None })
            } else {
                Err(anyhow!(
                    "`{}` matches {} directories",
                    dir.display(),
                    directories.len()
                ))
            }
        } else {
            Err(anyhow!("`{}` is not a dir", dir.display()))
        }
    }

    /// Install this `Source`.
    fn lock(self, ctx: &Context) -> Result<LockedSource> {
        match self {
            Self::Git { url, reference } => {
                let mut dir = ctx.clone_dir().to_path_buf();
                dir.push(
                    url.host_str()
                        .with_context(s!("URL `{}` has no host", url))?,
                );
                dir.push(url.path().trim_start_matches('/'));
                Self::lock_git(ctx, dir, url, reference.into())
            }
            Self::Remote { url } => {
                let mut dir = ctx.download_dir().to_path_buf();
                dir.push(
                    url.host_str()
                        .with_context(s!("URL `{}` has no host", url))?,
                );

                let segments: Vec<_> = url
                    .path_segments()
                    .with_context(s!("URL `{}` is cannot-be-a-base", url))?
                    .collect();
                let (base, rest) = segments.split_last().unwrap();
                let base = if *base == "" { "index" } else { *base };
                dir.push(rest.iter().collect::<PathBuf>());
                let file = dir.join(base);

                Self::lock_remote(ctx, dir, file, url)
            }
            Self::Local { dir } => Self::lock_local(ctx, dir),
        }
    }
}

impl ExternalPlugin {
    fn match_globs(dir: &Path, pattern: &str, files: &mut Vec<PathBuf>) -> Result<bool> {
        let mut matched = false;
        for entry in globwalk::GlobWalkerBuilder::new(dir, &pattern)
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
            .build()
            .with_context(s!("failed to parse glob pattern `{}`", pattern))?
        {
            files.push(
                entry
                    .with_context(s!("failed to read path matched by pattern `{}`", &pattern))?
                    .into_path(),
            );
            matched = true;
        }
        Ok(matched)
    }

    /// Consume the `ExternalPlugin` and convert it to a `LockedExternalPlugin`.
    fn lock(
        self,
        ctx: &Context,
        templates: &IndexMap<String, Template>,
        locked_source: LockedSource,
        global_matches: &[String],
        global_apply: &[String],
    ) -> Result<LockedExternalPlugin> {
        let Self {
            name,
            source,
            dir,
            uses,
            apply,
        } = self;

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
            data.insert("directory", dir_as_str);

            let mut files = Vec::new();

            // If the plugin defined what files to use, we do all of them.
            if let Some(uses) = &uses {
                for u in uses {
                    let pattern = hbs
                        .render_template(u, &data)
                        .with_context(s!("failed to render template `{}`", u))?;
                    if !Self::match_globs(dir, &pattern, &mut files)? {
                        bail!("failed to find any files matching `{}`", &pattern);
                    };
                }
            // Otherwise we try to figure out which files to use...
            } else {
                for g in global_matches {
                    let pattern = hbs
                        .render_template(g, &data)
                        .with_context(s!("failed to render template `{}`", g))?;
                    if Self::match_globs(dir, &pattern, &mut files)? {
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
}

impl Config {
    /// Consume the `Config` and convert it to a `LockedConfig`.
    ///
    /// This method installs all necessary remote dependencies of plugins,
    /// validates that local plugins are present, and checks that templates
    /// can compile.
    pub fn lock(self, ctx: &Context) -> Result<LockedConfig> {
        let Self {
            shell,
            matches,
            apply,
            templates,
            plugins,
        } = self;

        let templates = {
            let mut map = shell.default_templates().clone();
            for (name, template) in templates {
                map.insert(name, template);
            }
            map
        };

        // Partition the plugins into external and inline plugins.
        let (externals, inlines): (Vec<_>, Vec<_>) =
            plugins
                .into_iter()
                .enumerate()
                .partition_map(|(index, plugin)| match plugin {
                    Plugin::External(plugin) => Either::Left((index, plugin)),
                    Plugin::Inline(plugin) => Either::Right((index, LockedPlugin::Inline(plugin))),
                });

        // Create a map of unique `Source` to `Vec<Plugin>`
        let mut map = IndexMap::new();
        for (index, plugin) in externals {
            map.entry(plugin.source.clone())
                .or_insert_with(|| Vec::with_capacity(1))
                .push((index, plugin));
        }

        let matches = &matches.as_ref().unwrap_or_else(|| shell.default_matches());
        let apply = &apply.as_ref().unwrap_or_else(|| shell.default_apply());
        let count = map.len();
        let mut errors = Vec::new();

        let plugins = if count == 0 {
            inlines
                .into_iter()
                .map(|(_, locked)| locked)
                .collect::<Vec<_>>()
        } else {
            /// The maximmum number of threads to use while downloading sources.
            const MAX_THREADS: u32 = 8;

            // Create a thread pool and install the sources in parallel.
            let thread_count = cmp::min(count.try_into().unwrap_or(MAX_THREADS), MAX_THREADS);
            let mut pool = scoped_threadpool::Pool::new(thread_count);
            let (tx, rx) = sync::mpsc::channel();
            let templates_ref = &templates;

            pool.scoped(move |scoped| {
                for (source, plugins) in map {
                    let tx = tx.clone();
                    scoped.execute(move || {
                        tx.send((|| {
                            let source_name = source.to_string();
                            let source = source
                                .lock(ctx)
                                .with_context(s!("failed to install source `{}`", source_name))?;

                            let mut locked = Vec::with_capacity(plugins.len());
                            for (index, plugin) in plugins {
                                let name = plugin.name.clone();
                                locked.push((
                                    index,
                                    plugin
                                        .lock(ctx, templates_ref, source.clone(), matches, apply)
                                        .with_context(s!("failed to install plugin `{}`", name)),
                                ));
                            }
                            Ok(locked)
                        })())
                        .expect("oops! did main thread die?");
                    })
                }
                scoped.join_all();
            });

            rx.iter()
                // all threads must send a response
                .take(count)
                // The result of this is basically an `Iter<Result<Vec<(usize, Result)>, _>>`
                // The first thing we need to do is to filter out the failures and record the
                // errors that occurred while installing the source in our `errors` list.
                // Finally, we flatten the sub lists into a single iterator.
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|result| match result {
                    Ok(ok) => Some(ok),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                })
                .flatten()
                // The result of this is basically a `Iter<(usize, Result<LockedExternalPlugin>)`.
                // Similar to the above, we filter out the failures that
                // occurred during locking of individual plugins and record the
                // errors. Next, we combine this with the inline plugins which
                // didn't have to be installed. Finally we sort by the original index
                // to end up wih an iterator of `LockedPlugin`s which we can collect into a
                // `Vec<_>`.
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|(index, result)| match result {
                    Ok(plugin) => Some((index, LockedPlugin::External(plugin))),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                })
                .chain(inlines.into_iter())
                .sorted_by_key(|(index, _)| *index)
                .map(|(_, locked)| locked)
                .collect::<Vec<_>>()
        };

        Ok(LockedConfig {
            settings: ctx.settings().clone(),
            templates,
            errors,
            plugins,
        })
    }
}

impl LockedExternalPlugin {
    /// Return a reference to the plugin directory.
    fn dir(&self) -> &Path {
        self.plugin_dir.as_ref().unwrap_or(&self.source_dir)
    }
}

impl LockedConfig {
    /// Read a `LockedConfig` from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let locked: Self = toml::from_str(&String::from_utf8_lossy(
            &fs::read(&path)
                .with_context(s!("failed to read locked config from `{}`", path.display()))?,
        ))
        .context("failed to deserialize locked config")?;
        Ok(locked)
    }

    /// Verify that the `LockedConfig` is okay.
    pub fn verify(&self, ctx: &Context) -> bool {
        if &self.settings != ctx.settings() {
            return false;
        }
        for plugin in &self.plugins {
            match plugin {
                LockedPlugin::External(plugin) => {
                    if !plugin.dir().exists() {
                        return false;
                    }
                    for file in &plugin.files {
                        if !file.exists() {
                            return false;
                        }
                    }
                }
                LockedPlugin::Inline(_) => {}
            }
        }
        true
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

    /// Clean the clone and download directories.
    pub fn clean(&self, ctx: &Context, warnings: &mut Vec<Error>) {
        let clean_clone_dir = self
            .settings
            .clone_dir()
            .starts_with(self.settings.data_dir());
        let clean_download_dir = self
            .settings
            .download_dir()
            .starts_with(self.settings.data_dir());

        if !clean_clone_dir && !clean_download_dir {
            return;
        }

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
                    if f.starts_with(self.settings.download_dir()) {
                        Some(f.as_path())
                    } else {
                        None
                    }
                }));
            }
        }
        parent_dirs.insert(self.settings.clone_dir());
        parent_dirs.insert(self.settings.download_dir());

        if clean_clone_dir {
            for entry in WalkDir::new(self.settings.clone_dir())
                .into_iter()
                .filter_entry(|e| !source_dirs.contains(e.path()))
                .filter_map(result::Result::ok)
                .filter(|e| !parent_dirs.contains(e.path()))
            {
                if let Err(err) = Self::remove_path(ctx, entry.path()) {
                    warnings.push(err);
                }
            }
        }

        if clean_download_dir {
            for entry in WalkDir::new(self.settings.download_dir())
                .into_iter()
                .filter_map(result::Result::ok)
                .filter(|e| {
                    let p = e.path();
                    !files.contains(p) && !parent_dirs.contains(p)
                })
            {
                if let Err(err) = Self::remove_path(ctx, entry.path()) {
                    warnings.push(err);
                }
            }
        }
    }

    /// Generate the script.
    pub fn source(&self, ctx: &Context) -> Result<String> {
        // Compile the templates
        let mut templates = handlebars::Handlebars::new();
        templates.set_strict_mode(true);
        for (name, template) in &self.templates {
            templates
                .register_template_string(&name, &template.value)
                .with_context(s!("failed to compile template `{}`", name))?;
        }

        let mut script = String::new();

        for plugin in &self.plugins {
            match plugin {
                LockedPlugin::External(plugin) => {
                    for name in &plugin.apply {
                        let dir_as_str = plugin
                            .dir()
                            .to_str()
                            .context("plugin directory is not valid UTF-8")?;

                        // Data to use in template rendering
                        let mut data = hashmap! {
                            "data_dir" => self
                                .settings
                                .data_dir()
                                .to_str()
                                .context("data directory is not valid UTF-8")?,
                            "name" => &plugin.name,
                            "dir" => dir_as_str,
                            "directory" => dir_as_str,
                        };

                        if self.templates.get(name.as_str()).unwrap().each {
                            for file in &plugin.files {
                                let as_str =
                                    file.to_str().context("plugin file is not valid UTF-8")?;
                                data.insert("file", as_str);
                                data.insert("filename", as_str);
                                script.push_str(
                                    &templates
                                        .render(name, &data)
                                        .with_context(s!("failed to render template `{}`", name))?,
                                );
                                script.push('\n');
                            }
                        } else {
                            script.push_str(
                                &templates
                                    .render(name, &data)
                                    .with_context(s!("failed to render template `{}`", name))?,
                            );
                            script.push('\n');
                        }
                    }
                    status_v!(ctx, "Rendered", &plugin.name);
                }
                LockedPlugin::Inline(plugin) => {
                    let data = hashmap! {
                        "data_dir" => self
                            .settings
                            .data_dir()
                            .to_str()
                            .context("data directory is not valid UTF-8")?,
                        "name" => &plugin.name,
                    };
                    script.push_str(
                        &templates
                            .render_template(&plugin.raw, &data)
                            .with_context(s!(
                                "failed to render inline plugin `{}`",
                                &plugin.name
                            ))?,
                    );
                    script.push('\n');
                    status_v!(ctx, "Inlined", &plugin.name);
                }
            }
        }

        Ok(script)
    }

    /// Write a `LockedConfig` config to the given path.
    pub fn to_path<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        fs::write(
            path,
            &toml::to_string(&self).context("failed to serialize locked config")?,
        )
        .with_context(s!("failed to write locked config to `{}`", path.display()))?;
        Ok(())
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::io::{self, Read, Write};
    use std::process::Command;
    use std::thread;
    use std::time;

    use pretty_assertions::assert_eq;
    use url::Url;

    fn git_clone_sheldon_test(temp: &tempfile::TempDir) -> git2::Repository {
        let dir = temp.path();
        Command::new("git")
            .arg("clone")
            .arg("https://github.com/rossmacarthur/sheldon-test")
            .arg(&dir)
            .output()
            .expect("git clone rossmacarthur/sheldon-test");
        git2::Repository::open(dir).expect("open sheldon-test git repository")
    }

    fn create_test_context(root: &Path) -> Context {
        Context {
            settings: Settings {
                version: structopt::clap::crate_version!().to_string(),
                home: "/".into(),
                config_file: root.join("config.toml"),
                lock_file: root.join("config.lock"),
                clone_dir: root.join("repos"),
                download_dir: root.join("downloads"),
                data_dir: root.to_path_buf(),
                config_dir: root.to_path_buf(), // must come after the joins above
            },
            output: crate::log::Output {
                verbosity: crate::log::Verbosity::Quiet,
                no_color: true,
            },
            mode: Mode::Normal,
        }
    }

    fn read_file_contents(file: &Path) -> io::Result<String> {
        let mut file = fs::File::open(file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    #[test]
    fn git_checkout_to_string() {
        assert_eq!(
            GitCheckout::Branch("feature".to_string()).to_string(),
            "@feature"
        );
        assert_eq!(
            GitCheckout::Rev("ad149784a".to_string()).to_string(),
            "@ad149784a"
        );
        assert_eq!(GitCheckout::Tag("0.2.3".to_string()).to_string(), "@0.2.3");
    }

    #[test]
    fn git_checkout_resolve_branch() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let checkout = GitCheckout::Branch("feature".to_string());
        let oid = checkout.resolve(&repo).expect("lock git checkout");
        assert_eq!(oid.to_string(), "09ead574b20bb573ae0a53c1a5c546181cfa41c8");

        let checkout = GitCheckout::Branch("not-a-branch".to_string());
        let error = checkout.resolve(&repo).unwrap_err();
        assert_eq!(error.to_string(), "failed to find branch `not-a-branch`");
    }

    #[test]
    fn git_checkout_resolve_rev() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let checkout = GitCheckout::Rev("ad149784a".to_string());
        let oid = checkout.resolve(&repo).unwrap();
        assert_eq!(oid.to_string(), "ad149784a1538291f2477fb774eeeed4f4d29e45");

        let checkout = GitCheckout::Rev("2c4ed7710".to_string());
        let error = checkout.resolve(&repo).unwrap_err();
        assert_eq!(error.to_string(), "failed to find revision `2c4ed7710`");
    }

    #[test]
    fn git_checkout_resolve_tag() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let checkout = GitCheckout::Tag("v0.1.0".to_string());
        let oid = checkout.resolve(&repo).unwrap();
        assert_eq!(oid.to_string(), "be8fde277e76f35efbe46848fb352cee68549962");

        let checkout = GitCheckout::Tag("v0.2.0".to_string());
        let error = checkout.resolve(&repo).unwrap_err();
        assert_eq!(error.to_string(), "failed to find tag `v0.2.0`");
    }

    #[test]
    fn source_to_string() {
        assert_eq!(
            Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.3.0".to_string())),
            }
            .to_string(),
            "https://github.com/rossmacarthur/sheldon-test@v0.3.0"
        );
        assert_eq!(
            Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: None,
            }
            .to_string(),
            "https://github.com/rossmacarthur/sheldon-test"
        );
        assert_eq!(
            Source::Remote {
                url: Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT")
                    .unwrap(),
            }
            .to_string(),
            "https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT"
        );
        assert_eq!(
            Source::Local {
                dir: PathBuf::from("~/plugins")
            }
            .to_string(),
            "~/plugins"
        );
    }

    #[test]
    fn source_lock_git_and_reinstall() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let mut ctx = create_test_context(dir);
        let url = Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap();

        let locked = Source::lock_git(
            &ctx,
            dir.to_path_buf(),
            url.clone(),
            GitCheckout::DefaultBranch,
        )
        .unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(&dir).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );

        let modified = fs::metadata(&dir).unwrap().modified().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        ctx.mode = Mode::Reinstall;
        let locked =
            Source::lock_git(&ctx, dir.to_path_buf(), url, GitCheckout::DefaultBranch).unwrap();
        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(&dir).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );
        assert!(fs::metadata(&dir).unwrap().modified().unwrap() > modified);
    }

    #[test]
    fn source_lock_git_https_with_checkout() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();

        let locked = Source::lock_git(
            &create_test_context(dir),
            dir.to_path_buf(),
            Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
            GitCheckout::Rev("ad149784a1538291f2477fb774eeeed4f4d29e45".to_string()),
        )
        .unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(&dir).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(
            head.target().unwrap().to_string(),
            "ad149784a1538291f2477fb774eeeed4f4d29e45"
        )
    }

    #[test]
    fn source_lock_git_git_with_checkout() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();

        let locked = Source::lock_git(
            &create_test_context(dir),
            dir.to_path_buf(),
            Url::parse("git://github.com/rossmacarthur/sheldon-test").unwrap(),
            GitCheckout::Rev("ad149784a1538291f2477fb774eeeed4f4d29e45".to_string()),
        )
        .unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
        let repo = git2::Repository::open(&dir).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(
            head.target().unwrap().to_string(),
            "ad149784a1538291f2477fb774eeeed4f4d29e45"
        )
    }

    #[test]
    fn source_lock_remote_and_reinstall() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let file = dir.join("test.txt");
        let mut ctx = create_test_context(dir);
        let url =
            Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT").unwrap();

        let locked =
            Source::lock_remote(&ctx, dir.to_path_buf(), file.clone(), url.clone()).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, Some(file.clone()));
        assert_eq!(
            read_file_contents(&file).unwrap(),
            read_file_contents(&manifest_dir.join("LICENSE-MIT")).unwrap()
        );

        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        ctx.mode = Mode::Reinstall;
        let locked = Source::lock_remote(&ctx, dir.to_path_buf(), file.clone(), url).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, Some(file.clone()));
        assert_eq!(
            read_file_contents(&file).unwrap(),
            read_file_contents(&manifest_dir.join("LICENSE-MIT")).unwrap()
        );
        assert!(fs::metadata(&file).unwrap().modified().unwrap() > modified)
    }

    #[test]
    fn source_lock_local() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let _ = git_clone_sheldon_test(&temp);

        let locked = Source::lock_local(&create_test_context(dir), dir.to_path_buf()).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, None);
    }

    #[test]
    fn source_lock_with_git() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = create_test_context(dir);

        let source = Source::Git {
            url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
            reference: None,
        };
        let locked = source.lock(&ctx).unwrap();

        assert_eq!(
            locked.dir,
            dir.join("repos/github.com/rossmacarthur/sheldon-test")
        );
        assert_eq!(locked.file, None)
    }

    #[test]
    fn source_lock_with_remote() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = create_test_context(dir);

        let source = Source::Remote {
            url: Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT")
                .unwrap(),
        };
        let locked = source.lock(&ctx).unwrap();

        assert_eq!(
            locked.dir,
            dir.join("downloads/github.com/rossmacarthur/sheldon/raw/0.3.0")
        );
        assert_eq!(
            locked.file,
            Some(dir.join("downloads/github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT"))
        );
    }

    #[test]
    fn external_plugin_lock_git_with_uses() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = create_test_context(dir);
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
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let clone_dir = dir.join("repos/github.com/rossmacarthur/sheldon-test");

        let locked = plugin
            .lock(
                &ctx,
                &Shell::default().default_templates().clone(),
                locked_source,
                &[],
                &["hello".into()],
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
        let ctx = create_test_context(dir);
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
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let clone_dir = dir.join("repos/github.com/rossmacarthur/sheldon-test");

        let locked = plugin
            .lock(
                &ctx,
                &Shell::default().default_templates().clone(),
                locked_source,
                &["*.plugin.zsh".to_string()],
                &["hello".to_string()],
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
        let ctx = create_test_context(dir);
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
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();

        plugin
            .lock(
                &ctx,
                &Shell::default().default_templates().clone(),
                locked_source,
                &["*doesnotexist*".to_string()],
                &["source".to_string()],
            )
            .unwrap_err();
    }

    #[test]
    fn external_plugin_lock_git_with_matches_not_each() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = create_test_context(dir);
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
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let clone_dir = dir.join("repos/github.com/rossmacarthur/sheldon-test");

        let locked = plugin
            .lock(
                &ctx,
                &Shell::default().default_templates().clone(),
                locked_source,
                &["*doesnotexist*".to_string()],
                &["PATH".to_string()],
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
        let ctx = create_test_context(dir);
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
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let download_dir = dir.join("downloads/github.com/rossmacarthur/sheldon-test/raw/master");

        let locked = plugin
            .lock(
                &ctx,
                &Shell::default().default_templates().clone(),
                locked_source,
                &[],
                &["hello".to_string()],
            )
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.dir(), download_dir);
        assert_eq!(locked.files, vec![download_dir.join("test.plugin.zsh")]);
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn config_lock_empty() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let ctx = create_test_context(dir);
        let config = Config {
            shell: Shell::Zsh,
            matches: None,
            apply: None,
            templates: IndexMap::new(),
            plugins: Vec::new(),
        };

        let locked = config.lock(&ctx).unwrap();

        assert_eq!(&locked.settings, ctx.settings());
        assert_eq!(locked.plugins, Vec::new());
        assert_eq!(
            locked.templates,
            Shell::default().default_templates().clone(),
        );
        assert_eq!(locked.errors.len(), 0);
    }

    #[test]
    fn locked_config_clean() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let ctx = create_test_context(temp.path());
        let config = Config {
            shell: Shell::Zsh,
            matches: None,
            apply: None,
            templates: IndexMap::new(),
            plugins: vec![Plugin::External(ExternalPlugin {
                name: "test".to_string(),
                source: Source::Git {
                    url: Url::parse("git://github.com/rossmacarthur/sheldon-test").unwrap(),
                    reference: None,
                },
                dir: None,
                uses: None,
                apply: None,
            })],
        };
        let locked = config.lock(&ctx).unwrap();
        let test_dir = ctx.clone_dir().join("github.com/rossmacarthur/another-dir");
        let test_file = test_dir.join("test.txt");
        fs::create_dir_all(&test_dir).unwrap();
        {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&test_file)
                .unwrap();
        }

        let mut warnings = Vec::new();
        locked.clean(&ctx, &mut warnings);
        assert!(warnings.is_empty());
        assert!(ctx
            .clone_dir()
            .join("github.com/rossmacarthur/sheldon-test")
            .exists());
        assert!(ctx
            .clone_dir()
            .join("github.com/rossmacarthur/sheldon-test/test.plugin.zsh")
            .exists());
        assert!(!test_file.exists());
        assert!(!test_dir.exists());
    }

    #[test]
    fn locked_config_to_and_from_path() {
        let mut temp = tempfile::NamedTempFile::new().unwrap();
        let content = r#"version = "<version>"
home = "<home>"
config_dir = "<config>"
data_dir = "<data>"
config_file = "<config>/plugins.toml"
lock_file = "<data>/plugins.lock"
clone_dir = "<data>/repos"
download_dir = "<data>/downloads"
plugins = []

[templates]
"#;
        temp.write_all(content.as_bytes()).unwrap();
        let locked_config = LockedConfig::from_path(temp.into_temp_path()).unwrap();
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.into_temp_path();
        locked_config.to_path(&path).unwrap();
        assert_eq!(read_file_contents(&path).unwrap(), content);
    }
}
