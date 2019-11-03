//! Plugin installation.
//!
//! This module handles the downloading of `Source`s and figuring out which
//! filenames to use for `Plugins`.

use std::{
    cmp,
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    result, sync,
};

use indexmap::{indexmap, IndexMap};
use itertools::{Either, Itertools};
use lazy_static::lazy_static;
use maplit::hashmap;
use serde::{Deserialize, Serialize};
use url::Url;
use walkdir::WalkDir;

use crate::{
    config::{Config, ExternalPlugin, GitReference, InlinePlugin, Plugin, Source, Template},
    context::Context,
    util::git,
    Error, Result, ResultExt,
};

/// The maximmum number of threads to use while downloading sources.
const MAX_THREADS: u32 = 8;

lazy_static! {
    /// The default template names to apply.
    pub static ref DEFAULT_APPLY: Vec<String> = vec_into!["source"];
}

lazy_static! {
    /// The default templates.
    pub static ref DEFAULT_TEMPLATES: IndexMap<String, Template> = indexmap_into! {
        "PATH" => "export PATH=\"{{ directory }}:$PATH\"",
        "path" => "path=( \"{{ directory }}\" $path )",
        "fpath" => "fpath=( \"{{ directory }}\" $fpath )",
        "source" => Template::from("source \"{{ filename }}\"").each(true)
    };
}

/////////////////////////////////////////////////////////////////////////
// Locked configuration definitions
/////////////////////////////////////////////////////////////////////////

/// A locked `GitReference`.
#[derive(Clone, Debug)]
struct LockedGitReference(git2::Oid);

/// A locked `Source`.
#[derive(Clone, Debug)]
struct LockedSource {
    /// The clone or download directory.
    directory: PathBuf,
    /// The download filename.
    filename: Option<PathBuf>,
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
    /// The filenames to use in the plugin directory.
    filenames: Vec<PathBuf>,
    /// What templates to apply to each filename.
    apply: Vec<String>,
}

/// A locked `Plugin`.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
enum LockedPlugin {
    External(LockedExternalPlugin),
    Inline(InlinePlugin),
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct LockedContext {
    /// The current crate version.
    version: String,
    /// The location of the home directory.
    home: PathBuf,
    /// The location of the root directory.
    root: PathBuf,
    /// The location of the config file.
    config_file: PathBuf,
    /// The location of the lock file.
    lock_file: PathBuf,
    /// The directory to clone git sources to.
    clone_dir: PathBuf,
    /// The directory to download remote plugins sources to.
    download_dir: PathBuf,
}

/// A locked `Config`.
#[derive(Debug, Deserialize, Serialize)]
pub struct LockedConfig {
    /// The global context that was used to generated this `LockedConfig`.
    #[serde(flatten)]
    pub ctx: LockedContext,
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

impl PartialEq<Context> for LockedContext {
    fn eq(&self, other: &Context) -> bool {
        self.version == other.version
            && self.home == other.home
            && self.root == other.root
            && self.config_file == other.config_file
            && self.lock_file == other.lock_file
            && self.clone_dir == other.clone_dir
            && self.download_dir == other.download_dir
    }
}

impl fmt::Display for GitReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Branch(s) | Self::Revision(s) | Self::Tag(s) => write!(f, "{}", s),
        }
    }
}

impl GitReference {
    /// Consume the `GitReference` and convert it to a `LockedGitReference`.
    ///
    /// This code is take from [Cargo].
    ///
    /// [Cargo]: https://github.com/rust-lang/cargo/blob/master/src/cargo/sources/git/utils.rs#L207-L232
    fn lock(&self, repo: &git2::Repository) -> Result<LockedGitReference> {
        match self {
            Self::Branch(s) => git::resolve_branch(repo, s),
            Self::Revision(s) => git::resolve_revision(repo, s),
            Self::Tag(s) => git::resolve_tag(repo, s),
        }
        .map(LockedGitReference)
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Git {
                url,
                reference: Some(reference),
            } => write!(f, "{}@{}", url, reference),
            Self::Git { url, .. } | Self::Remote { url } => write!(f, "{}", url),
            Self::Local { directory } => write!(f, "{}", directory.display()),
        }
    }
}

impl Source {
    /// Clones a Git repository and checks it out at a particular revision.
    fn lock_git(
        ctx: &Context,
        directory: PathBuf,
        url: Url,
        reference: Option<GitReference>,
    ) -> Result<LockedSource> {
        if ctx.reinstall {
            if let Err(e) = fs::remove_dir_all(&directory) {
                if e.kind() != io::ErrorKind::NotFound {
                    return Err(e)
                        .chain(s!("failed to remove directory `{}`", &directory.display()));
                }
            }
        }

        let (cloned, repo) = git::clone_or_open(&url, &directory)?;
        let status = if cloned { "Cloned" } else { "Checked" };

        // Checkout the configured revision.
        if let Some(reference) = reference {
            git::checkout(&repo, reference.lock(&repo)?.0)?;
            ctx.status(status, &format!("{}@{}", &url, reference));
        } else {
            ctx.status(status, &url);
        }

        // Recursively update Git submodules.
        git::submodule_update(&repo).chain("failed to recursively update submodules")?;

        Ok(LockedSource {
            directory,
            filename: None,
        })
    }

    /// Downloads a Remote source.
    fn lock_remote(
        ctx: &Context,
        directory: PathBuf,
        filename: PathBuf,
        url: Url,
    ) -> Result<LockedSource> {
        if ctx.reinstall {
            if let Err(e) = fs::remove_file(&filename) {
                if e.kind() != io::ErrorKind::NotFound {
                    return Err(e).chain(s!("failed to remove filename `{}`", &filename.display()));
                }
            }
        }

        if !filename.exists() {
            fs::create_dir_all(&directory)
                .chain(s!("failed to create directory `{}`", directory.display()))?;
            let mut response =
                reqwest::get(url.clone()).chain(s!("failed to download `{}`", url))?;
            let mut out = fs::File::create(&filename)
                .chain(s!("failed to create `{}`", filename.display()))?;
            io::copy(&mut response, &mut out)
                .chain(s!("failed to copy contents to `{}`", filename.display()))?;
            ctx.status("Fetched", &url);
        } else {
            ctx.status("Checked", &url);
        }

        Ok(LockedSource {
            directory,
            filename: Some(filename),
        })
    }

    /// Checks that a Local source directory exists.
    fn lock_local(ctx: &Context, directory: PathBuf) -> Result<LockedSource> {
        let directory = ctx.expand_tilde(directory);

        if let Ok(glob) = glob::glob(&directory.to_string_lossy()) {
            let mut directories: Vec<_> = glob
                .filter_map(|result| {
                    if let Ok(directory) = result {
                        if directory.is_dir() {
                            return Some(directory);
                        }
                    }
                    None
                })
                .collect();

            if directories.len() == 1 {
                let directory = directories.remove(0);
                ctx.status("Checked", &ctx.replace_home(&directory).display());
                Ok(LockedSource {
                    directory,
                    filename: None,
                })
            } else {
                err!(
                    "`{}` matches {} directories",
                    directory.display(),
                    directories.len()
                )
            }
        } else if fs::metadata(&directory)
            .chain(s!("failed to find directory `{}`", directory.display()))?
            .is_dir()
        {
            ctx.status("Checked", &ctx.replace_home(&directory).display());
            Ok(LockedSource {
                directory,
                filename: None,
            })
        } else {
            err!("`{}` is not a directory", directory.display())
        }
    }

    /// Install this `Source`.
    fn lock(self, ctx: &Context) -> Result<LockedSource> {
        match self {
            Self::Git { url, reference } => {
                let mut directory = ctx.clone_dir.clone();
                directory.push(url.host_str().chain(s!("URL `{}` has no host", url))?);
                directory.push(url.path().trim_start_matches('/'));
                Self::lock_git(ctx, directory, url, reference)
            }
            Self::Remote { url } => {
                let mut directory = ctx.download_dir.clone();
                directory.push(url.host_str().chain(s!("URL `{}` has no host", url))?);

                let segments: Vec<_> = url
                    .path_segments()
                    .chain(s!("URL `{}` is cannot-be-a-base", url))?
                    .collect();
                let (base, rest) = segments.split_last().unwrap();
                let base = if *base != "" { *base } else { "index" };
                directory.push(rest.iter().collect::<PathBuf>());
                let filename = directory.join(base);

                Self::lock_remote(ctx, directory, filename, url)
            }
            Self::Local { directory } => Self::lock_local(ctx, directory),
        }
    }
}

impl ExternalPlugin {
    fn match_globs(pattern: PathBuf, filenames: &mut Vec<PathBuf>) -> Result<bool> {
        let mut matched = false;
        let pattern = pattern.to_string_lossy();
        let paths: glob::Paths =
            glob::glob(&pattern).chain(s!("failed to parse glob pattern `{}`", &pattern))?;

        for path in paths {
            filenames
                .push(path.chain(s!("failed to read path matched by pattern `{}`", &pattern))?);
            matched = true;
        }

        Ok(matched)
    }

    /// Consume the `ExternalPlugin` and convert it to a `LockedExternalPlugin`.
    fn lock(
        self,
        ctx: &Context,
        source: LockedSource,
        matches: &[String],
        apply: &[String],
    ) -> Result<LockedExternalPlugin> {
        Ok(if let Source::Remote { .. } = self.source {
            let LockedSource {
                directory,
                filename,
            } = source;
            LockedExternalPlugin {
                name: self.name,
                source_dir: directory,
                plugin_dir: None,
                filenames: vec![filename.unwrap()],
                apply: self.apply.unwrap_or_else(|| apply.to_vec()),
            }
        } else {
            // Handlebars instance to do the rendering
            let mut templates = handlebars::Handlebars::new();
            templates.set_strict_mode(true);

            // Data to use in template rendering
            let mut data = hashmap! {
                "root" => ctx
                    .root
                    .to_str()
                    .chain("root directory is not valid UTF-8")?,
                "name" => &self.name
            };

            let source_dir = source.directory;
            let plugin_dir = if let Some(directory) = self.directory {
                let rendered = templates
                    .render_template(&directory, &data)
                    .chain(s!("failed to render template `{}`", directory))?;
                Some(source_dir.join(rendered))
            } else {
                None
            };
            let directory = plugin_dir.as_ref().unwrap_or(&source_dir);

            data.insert(
                "directory",
                &directory
                    .to_str()
                    .chain("plugin directory is not valid UTF-8")?,
            );

            let mut filenames = Vec::new();

            // If the plugin defined what files to use, we do all of them.
            if let Some(uses) = &self.uses {
                for u in uses {
                    let rendered = templates
                        .render_template(u, &data)
                        .chain(s!("failed to render template `{}`", u))?;
                    let pattern = directory.join(&rendered);
                    if !Self::match_globs(pattern, &mut filenames)? {
                        bail!("failed to find any files matching `{}`", &rendered);
                    };
                }
            // Otherwise we try to figure out which files to use...
            } else {
                for g in matches {
                    let rendered = templates
                        .render_template(g, &data)
                        .chain(s!("failed to render template `{}`", g))?;
                    let pattern = directory.join(rendered);
                    if Self::match_globs(pattern, &mut filenames)? {
                        break;
                    }
                }
            }

            LockedExternalPlugin {
                name: self.name,
                source_dir,
                plugin_dir,
                filenames,
                apply: self.apply.unwrap_or_else(|| apply.to_vec()),
            }
        })
    }
}

impl Context {
    /// Convert this `Context` to a `LockedContext`.
    fn lock(&self) -> LockedContext {
        LockedContext {
            version: self.version.to_string(),
            home: self.home.clone(),
            root: self.root.clone(),
            config_file: self.config_file.clone(),
            lock_file: self.lock_file.clone(),
            clone_dir: self.clone_dir.clone(),
            download_dir: self.download_dir.clone(),
        }
    }
}

impl Config {
    /// Consume the `Config` and convert it to a `LockedConfig`.
    ///
    /// This method installs all necessary remote dependencies of plugins,
    /// validates that local plugins are present, and checks that templates
    /// can compile.
    pub fn lock(self, ctx: &Context) -> Result<LockedConfig> {
        // Partition the plugins into external and inline plugins.
        let (externals, inlines): (Vec<_>, Vec<_>) = self
            .plugins
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

        let matches = &self.matches;
        let apply = &self.apply.as_ref().unwrap_or(&*DEFAULT_APPLY);
        let count = map.len();
        let mut errors = Vec::new();

        let plugins = if count == 0 {
            inlines
                .into_iter()
                .map(|(_, locked)| locked)
                .collect::<Vec<_>>()
        } else {
            // Create a thread pool and install the sources in parallel.
            let mut pool = scoped_threadpool::Pool::new(cmp::min(count as u32, MAX_THREADS));
            let (tx, rx) = sync::mpsc::channel();

            pool.scoped(|scoped| {
                for (source, plugins) in map {
                    let tx = tx.clone();
                    scoped.execute(move || {
                        tx.send((|| {
                            let source_name = format!("{}", source);
                            let source = source
                                .lock(ctx)
                                .chain(s!("failed to install source `{}`", source_name))?;

                            let mut locked = Vec::with_capacity(plugins.len());
                            for (index, plugin) in plugins {
                                let name = plugin.name.clone();
                                locked.push((
                                    index,
                                    plugin
                                        .lock(ctx, source.clone(), matches, apply)
                                        .chain(s!("failed to install plugin `{}`", name)),
                                ));
                            }

                            Ok(locked)
                        })())
                        .expect("oops! did main thread die?");
                    })
                }
                scoped.join_all();
            });

            rx
                .iter()
                // all threads must send a response
                .take(count)
                // collect into a `Vec<_>`
                .collect::<Vec<_>>()
                // iterate over the `Vec<Result<_>>`
                .into_iter()
                // store `Err`s and filter them out
                .filter_map(|result| match result {
                    Ok(ok) => Some(ok),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                })
                // flatten the `Iter<Vec<_>>`
                .flatten()
                // collect into a `Vec<_>`
                .collect::<Vec<_>>()
                // iterate over the `Vec<(index, Result<LockedExternalPlugin>)>>`
                .into_iter()
                // store `Err`s and filter them out
                .filter_map(|(index, result)| match result {
                    Ok(plugin) => Some((index, LockedPlugin::External(plugin))),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                })
                // chain inline plugins
                .chain(inlines.into_iter())
                // sort by the original index
                .sorted_by_key(|(index, _)| *index)
                // remove the index
                .map(|(_, locked)| locked)
                // finally collect into a `Vec<LockedPlugin>`
                .collect::<Vec<_>>()
        };

        Ok(LockedConfig {
            ctx: ctx.lock(),
            templates: self.templates,
            errors,
            plugins,
        })
    }
}

impl LockedExternalPlugin {
    /// Return a reference to the plugin directory.
    fn directory(&self) -> &Path {
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
            &fs::read(&path).chain(s!("failed to read locked config from `{}`", path.display()))?,
        ))
        .chain("failed to deserialize locked config")?;
        Ok(locked)
    }

    /// Verify that the `LockedConfig` is okay.
    pub fn verify(&self, ctx: &Context) -> bool {
        if &self.ctx != ctx {
            return false;
        }
        for plugin in &self.plugins {
            match plugin {
                LockedPlugin::External(plugin) => {
                    if !plugin.directory().exists() {
                        return false;
                    }
                    for filename in &plugin.filenames {
                        if !filename.exists() {
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
            .chain(s!("failed to fetch metadata for `{}`", path_display))?
            .is_dir()
        {
            fs::remove_dir_all(path).chain(s!("failed to remove directory `{}`", path_display))?;
        } else {
            fs::remove_file(path).chain(s!("failed to remove file `{}`", path_display))?;
        }
        ctx.warning_v("Removed", path_display);
        Ok(())
    }

    /// Clean the clone and download directories.
    pub fn clean(&self, ctx: &Context) -> Vec<Error> {
        let mut warnings = Vec::new();
        let clean_clone_dir = self.ctx.clone_dir.starts_with(&self.ctx.root);
        let clean_download_dir = self.ctx.download_dir.starts_with(&self.ctx.root);

        if !clean_clone_dir && !clean_download_dir {
            return warnings;
        }

        // Track the source directories, all the plugin directory parents, and all the
        // plugin filenames.
        let mut source_dirs = HashSet::new();
        let mut parent_dirs = HashSet::new();
        let mut filenames = HashSet::new();
        for plugin in &self.plugins {
            if let LockedPlugin::External(locked) = plugin {
                source_dirs.insert(locked.source_dir.as_path());
                parent_dirs.extend(locked.directory().ancestors());
                filenames.extend(locked.filenames.iter().filter_map(|f| {
                    // `filenames` is only used when filtering the download directory
                    if f.starts_with(&self.ctx.download_dir) {
                        Some(f.as_path())
                    } else {
                        None
                    }
                }));
            }
        }
        parent_dirs.insert(self.ctx.clone_dir.as_path());
        parent_dirs.insert(self.ctx.download_dir.as_path());

        if clean_clone_dir {
            for entry in WalkDir::new(&self.ctx.clone_dir)
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
            for entry in WalkDir::new(&self.ctx.download_dir)
                .into_iter()
                .filter_map(result::Result::ok)
                .filter(|e| {
                    let p = e.path();
                    !filenames.contains(p) && !parent_dirs.contains(p)
                })
            {
                if let Err(err) = Self::remove_path(ctx, entry.path()) {
                    warnings.push(err);
                }
            }
        }

        warnings
    }

    /// Generate the script.
    pub fn source(&self, ctx: &Context) -> Result<String> {
        // Collaborate the default templates and the configured ones.
        let mut templates_map: HashMap<&str, &Template> =
            HashMap::with_capacity(DEFAULT_TEMPLATES.len() + self.templates.len());
        for (name, template) in DEFAULT_TEMPLATES.iter() {
            templates_map.insert(name, template);
        }
        for (name, template) in &self.templates {
            templates_map.insert(name, template);
        }

        // Compile the templates
        let mut templates = handlebars::Handlebars::new();
        templates.set_strict_mode(true);
        for (name, template) in &templates_map {
            templates
                .register_template_string(&name, &template.value)
                .chain(s!("failed to compile template `{}`", name))?;
        }

        let mut script = String::new();

        for plugin in &self.plugins {
            match plugin {
                LockedPlugin::External(plugin) => {
                    for name in &plugin.apply {
                        // Data to use in template rendering
                        let mut data = hashmap! {
                            "root" => self
                                .ctx.root
                                .to_str()
                                .chain("root directory is not valid UTF-8")?,
                            "name" => &plugin.name,
                            "directory" => plugin
                                .directory()
                                .to_str()
                                .chain("plugin directory is not valid UTF-8")?,
                        };

                        if templates_map.get(name.as_str()).unwrap().each {
                            for filename in &plugin.filenames {
                                data.insert(
                                    "filename",
                                    filename
                                        .to_str()
                                        .chain("plugin filename is not valid UTF-8")?,
                                );
                                script.push_str(
                                    &templates
                                        .render(name, &data)
                                        .chain(s!("failed to render template `{}`", name))?,
                                );
                                script.push('\n');
                            }
                        } else {
                            script.push_str(
                                &templates
                                    .render(name, &data)
                                    .chain(s!("failed to render template `{}`", name))?,
                            );
                            script.push('\n');
                        }
                    }
                    ctx.status_v("Rendered", &plugin.name);
                }
                LockedPlugin::Inline(plugin) => {
                    let data = hashmap! {
                        "root" => self
                            .ctx.root
                            .to_str()
                            .chain("root directory is not valid UTF-8")?,
                        "name" => &plugin.name,
                    };
                    script.push_str(
                        &templates
                            .render_template(&plugin.raw, &data)
                            .chain(s!("failed to render inline plugin `{}`", &plugin.name))?,
                    );
                    script.push('\n');
                    ctx.status_v("Inlined", &plugin.name);
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
            &toml::to_string(&self).chain("failed to serialize locked config")?,
        )
        .chain(s!("failed to write locked config to `{}`", path.display()))?;
        Ok(())
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        process::Command,
        thread, time,
    };
    use url::Url;

    fn git_clone_sheldon_test(temp: &tempfile::TempDir) -> git2::Repository {
        let directory = temp.path();
        Command::new("git")
            .arg("clone")
            .arg("https://github.com/rossmacarthur/sheldon-test")
            .arg(&directory)
            .output()
            .expect("git clone rossmacarthur/sheldon-test");
        git2::Repository::open(directory).expect("open sheldon-test git repository")
    }

    fn create_test_context(root: &Path) -> Context {
        Context {
            version: clap::crate_version!(),
            verbosity: crate::Verbosity::Quiet,
            no_color: true,
            home: "/".into(),
            config_file: root.join("config.toml"),
            lock_file: root.join("config.lock"),
            clone_dir: root.join("repositories"),
            download_dir: root.join("downloads"),
            root: root.to_path_buf(), // must come after the joins above
            command: crate::Command::Lock,
            reinstall: false,
            relock: false,
        }
    }

    fn read_file_contents(filename: &Path) -> io::Result<String> {
        let mut file = fs::File::open(filename)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    #[test]
    fn git_reference_to_string() {
        assert_eq!(
            GitReference::Branch("feature".to_string()).to_string(),
            "feature"
        );
        assert_eq!(
            GitReference::Revision("ad149784a".to_string()).to_string(),
            "ad149784a"
        );
        assert_eq!(GitReference::Tag("0.2.3".to_string()).to_string(), "0.2.3");
    }

    #[test]
    fn git_reference_lock_branch() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let reference = GitReference::Branch("feature".to_string());
        let locked = reference.lock(&repo).expect("lock git reference");
        assert_eq!(
            locked.0.to_string(),
            "09ead574b20bb573ae0a53c1a5c546181cfa41c8"
        );

        let reference = GitReference::Branch("not-a-branch".to_string());
        let error = reference.lock(&repo).unwrap_err();
        assert_eq!(
            error.to_string(),
            "failed to find branch `not-a-branch`\ncannot locate remote-tracking branch \
             \'origin/not-a-branch\'; class=Reference (4); code=NotFound (-3)"
        );
    }

    #[test]
    fn git_reference_lock_revision() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let reference = GitReference::Revision("ad149784a".to_string());
        let locked = reference.lock(&repo).unwrap();
        assert_eq!(
            locked.0.to_string(),
            "ad149784a1538291f2477fb774eeeed4f4d29e45"
        );

        let reference = GitReference::Revision("2c4ed7710".to_string());
        let error = reference.lock(&repo).unwrap_err();
        assert_eq!(
            error.to_string(),
            "failed to find revision `2c4ed7710`\nrevspec \'2c4ed7710\' not found; \
             class=Reference (4); code=NotFound (-3)"
        );
    }

    #[test]
    fn git_reference_lock_tag() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let repo = git_clone_sheldon_test(&temp);

        let reference = GitReference::Tag("v0.1.0".to_string());
        let locked = reference.lock(&repo).unwrap();
        assert_eq!(
            locked.0.to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );

        let reference = GitReference::Tag("v0.2.0".to_string());
        let error = reference.lock(&repo).unwrap_err();
        assert_eq!(
            error.to_string(),
            "failed to find tag `v0.2.0`\nreference \'refs/tags/v0.2.0\' not found; \
             class=Reference (4); code=NotFound (-3)"
        );
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
                directory: PathBuf::from("~/plugins")
            }
            .to_string(),
            "~/plugins"
        );
    }

    #[test]
    fn source_lock_git_and_reinstall() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let mut ctx = create_test_context(directory);
        let url = Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap();

        let locked = Source::lock_git(&ctx, directory.to_path_buf(), url.clone(), None).unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
        let repo = git2::Repository::open(&directory).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );

        let modified = fs::metadata(&directory).unwrap().modified().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        ctx.reinstall = true;
        let locked = Source::lock_git(&ctx, directory.to_path_buf(), url, None).unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
        let repo = git2::Repository::open(&directory).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            "be8fde277e76f35efbe46848fb352cee68549962"
        );
        assert!(fs::metadata(&directory).unwrap().modified().unwrap() > modified)
    }

    #[test]
    fn source_lock_git_with_reference() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();

        let locked = Source::lock_git(
            &create_test_context(directory),
            directory.to_path_buf(),
            Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
            Some(GitReference::Revision(
                "ad149784a1538291f2477fb774eeeed4f4d29e45".to_string(),
            )),
        )
        .unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
        let repo = git2::Repository::open(&directory).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(
            head.target().unwrap().to_string(),
            "ad149784a1538291f2477fb774eeeed4f4d29e45"
        )
    }

    #[test]
    fn source_lock_git_with_git() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();

        let locked = Source::lock_git(
            &create_test_context(directory),
            directory.to_path_buf(),
            Url::parse("git://github.com/rossmacarthur/sheldon-test").unwrap(),
            Some(GitReference::Revision(
                "ad149784a1538291f2477fb774eeeed4f4d29e45".to_string(),
            )),
        )
        .unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
        let repo = git2::Repository::open(&directory).unwrap();
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
        let directory = temp.path();
        let filename = directory.join("test.txt");
        let mut ctx = create_test_context(directory);
        let url =
            Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT").unwrap();

        let locked =
            Source::lock_remote(&ctx, directory.to_path_buf(), filename.clone(), url.clone())
                .unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, Some(filename.clone()));
        assert_eq!(
            read_file_contents(&filename).unwrap(),
            read_file_contents(&manifest_dir.join("LICENSE-MIT")).unwrap()
        );

        let modified = fs::metadata(&filename).unwrap().modified().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        ctx.reinstall = true;
        let locked =
            Source::lock_remote(&ctx, directory.to_path_buf(), filename.clone(), url).unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, Some(filename.clone()));
        assert_eq!(
            read_file_contents(&filename).unwrap(),
            read_file_contents(&manifest_dir.join("LICENSE-MIT")).unwrap()
        );
        assert!(fs::metadata(&filename).unwrap().modified().unwrap() > modified)
    }

    #[test]
    fn source_lock_local() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let _ = git_clone_sheldon_test(&temp);

        let locked =
            Source::lock_local(&create_test_context(directory), directory.to_path_buf()).unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
    }

    #[test]
    fn source_lock_with_git() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let ctx = create_test_context(directory);

        let source = Source::Git {
            url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
            reference: None,
        };
        let locked = source.lock(&ctx).unwrap();

        assert_eq!(
            locked.directory,
            directory.join("repositories/github.com/rossmacarthur/sheldon-test")
        );
        assert_eq!(locked.filename, None)
    }

    #[test]
    fn source_lock_with_remote() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let ctx = create_test_context(directory);

        let source = Source::Remote {
            url: Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT")
                .unwrap(),
        };
        let locked = source.lock(&ctx).unwrap();

        assert_eq!(
            locked.directory,
            directory.join("downloads/github.com/rossmacarthur/sheldon/raw/0.3.0")
        );
        assert_eq!(
            locked.filename,
            Some(
                directory.join("downloads/github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT")
            )
        );
    }

    #[test]
    fn external_plugin_lock_git_with_uses() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let ctx = create_test_context(directory);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.1.0".to_string())),
            },
            directory: None,
            uses: Some(vec!["*.md".into(), "{{ name }}.plugin.zsh".into()]),
            apply: None,
        };
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let clone_directory = directory.join("repositories/github.com/rossmacarthur/sheldon-test");

        let locked = plugin
            .lock(&ctx, locked_source, &[], &["hello".into()])
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.directory(), clone_directory);
        assert_eq!(
            locked.filenames,
            vec![
                clone_directory.join("README.md"),
                clone_directory.join("test.plugin.zsh")
            ]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn external_plugin_lock_git_with_matches() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let ctx = create_test_context(directory);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon-test").unwrap(),
                reference: Some(GitReference::Tag("v0.1.0".to_string())),
            },
            directory: None,
            uses: None,
            apply: None,
        };
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let clone_directory = directory.join("repositories/github.com/rossmacarthur/sheldon-test");

        let locked = plugin
            .lock(
                &ctx,
                locked_source,
                &["*.plugin.zsh".to_string()],
                &["hello".to_string()],
            )
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.directory(), clone_directory);
        assert_eq!(
            locked.filenames,
            vec![clone_directory.join("test.plugin.zsh")]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn external_plugin_lock_remote() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let ctx = create_test_context(directory);
        let plugin = ExternalPlugin {
            name: "test".to_string(),
            source: Source::Remote {
                url: Url::parse(
                    "https://github.com/rossmacarthur/sheldon-test/raw/master/test.plugin.zsh",
                )
                .unwrap(),
            },
            directory: None,
            uses: None,
            apply: None,
        };
        let locked_source = plugin.source.clone().lock(&ctx).unwrap();
        let download_directory =
            directory.join("downloads/github.com/rossmacarthur/sheldon-test/raw/master");

        let locked = plugin
            .lock(&ctx, locked_source, &[], &["hello".to_string()])
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.directory(), download_directory);
        assert_eq!(
            locked.filenames,
            vec![download_directory.join("test.plugin.zsh")]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn config_lock_empty() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let directory = temp.path();
        let ctx = create_test_context(directory);
        let config = Config {
            matches: Vec::new(),
            apply: None,
            templates: IndexMap::new(),
            plugins: Vec::new(),
        };

        let locked = config.lock(&ctx).unwrap();

        assert_eq!(
            locked.ctx,
            LockedContext {
                version: clap::crate_version!().to_string(),
                home: PathBuf::from("/"),
                root: directory.to_path_buf(),
                config_file: directory.join("config.toml"),
                lock_file: directory.join("config.lock"),
                clone_dir: directory.join("repositories"),
                download_dir: directory.join("downloads"),
            }
        );
        assert_eq!(locked.plugins, Vec::new());
        assert_eq!(locked.templates, IndexMap::new());
        assert_eq!(locked.errors.len(), 0);
    }

    #[test]
    fn config_lock_example_config() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let temp0 = tempfile::tempdir().expect("create temporary directory");
        let local_dir = temp0.path();
        let _ = git_clone_sheldon_test(&temp0);

        let temp1 = tempfile::tempdir().expect("create temporary directory");
        let root = temp1.path();
        let config_file = manifest_dir.join("docs/plugins.example.toml");
        let lock_file = root.join("plugins.lock");
        let clone_dir = root.join("repositories");
        let download_dir = root.join("downloads");
        let ctx = Context {
            version: clap::crate_version!(),
            verbosity: crate::Verbosity::Quiet,
            no_color: true,
            home: "/".into(),
            root: root.to_path_buf(),
            config_file: config_file.clone(),
            lock_file: lock_file.clone(),
            clone_dir: clone_dir.clone(),
            download_dir: download_dir.clone(),
            command: crate::Command::Lock,
            reinstall: false,
            relock: false,
        };

        let mut config = Config::from_path(&ctx.config_file).unwrap();
        {
            match &mut config.plugins[2] {
                Plugin::External(ref mut plugin) => {
                    plugin.name = "sheldon-test".to_string();
                    plugin.source = Source::Local {
                        directory: local_dir.to_path_buf(),
                    };
                }
                _ => panic!("expected the 3rd plugin to be external"),
            }
        }

        let locked = config.lock(&ctx).unwrap();

        assert_eq!(
            locked.ctx,
            LockedContext {
                version: clap::crate_version!().to_string(),
                home: PathBuf::from("/"),
                root: root.to_path_buf(),
                config_file,
                lock_file,
                clone_dir,
                download_dir
            }
        );
        assert_eq!(
            locked.plugins,
            vec![
                LockedPlugin::External(LockedExternalPlugin {
                    name: "async".to_string(),
                    source_dir: root.join("repositories/github.com/mafredri/zsh-async"),
                    plugin_dir: None,
                    filenames: vec![
                        root.join("repositories/github.com/mafredri/zsh-async/async.zsh")
                    ],
                    apply: vec_into!["function"]
                }),
                LockedPlugin::External(LockedExternalPlugin {
                    name: "pure".to_string(),
                    source_dir: root.join("repositories/github.com/sindresorhus/pure"),
                    plugin_dir: None,
                    filenames: vec![root.join("repositories/github.com/sindresorhus/pure/pure.zsh")],
                    apply: vec_into!["prompt"]
                }),
                LockedPlugin::External(LockedExternalPlugin {
                    name: "sheldon-test".to_string(),
                    source_dir: local_dir.to_path_buf(),
                    plugin_dir: None,
                    filenames: vec![root.join(local_dir.join("test.plugin.zsh"))],
                    apply: vec_into!["PATH", "source"]
                }),
                LockedPlugin::Inline(InlinePlugin {
                    name: "ip-netns".to_string(),
                    raw: r#"# Get ip netns information
ip_netns_prompt_info() {
  if (( $+commands[ip] )); then
    local ref="$(ip netns identify $$)"
    if [[ ! -z "$ref" ]]; then
      echo "${ZSH_THEME_IP_NETNS_PREFIX:=(}${ref}${ZSH_THEME_IP_NETNS_SUFFIX:=)}"
    fi
  fi
}
"#
                    .to_string()
                }),
                LockedPlugin::External(LockedExternalPlugin {
                    name: "docker-destroy-all".to_string(),
                    source_dir: root.join("repositories/gist.github.com/79ee61f7c140c63d2786"),
                    plugin_dir: None,
                    filenames: vec![root.join(
                        "repositories/gist.github.com/79ee61f7c140c63d2786/get_last_pane_path.sh"
                    )],
                    apply: vec_into!["PATH"]
                })
            ]
        );
        assert_eq!(
            locked.templates,
            indexmap_into![
                "function" => Template {
                    value: "ln -sf \"{{ filename }}\" \"{{ root }}/functions/{{ name }}\"".to_string(),
                    each: true
                },
                "prompt" => Template {
                    value:
                        "ln -sf \"{{ filename }}\" \"{{ root }}/functions/prompt_{{ name }}_setup\"".to_string(),
                    each: true
                }
            ]
        );
        assert_eq!(locked.errors.len(), 0);
    }

    #[test]
    fn locked_config_clean() {
        let temp = tempfile::tempdir().expect("create temporary directory");
        let ctx = create_test_context(temp.path());
        let config = Config {
            matches: vec_into!["*.zsh"],
            apply: None,
            templates: DEFAULT_TEMPLATES
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            plugins: vec![Plugin::External(ExternalPlugin {
                name: "test".to_string(),
                source: Source::Git {
                    url: Url::parse("git://github.com/rossmacarthur/sheldon-test").unwrap(),
                    reference: None,
                },
                directory: None,
                uses: None,
                apply: None,
            })],
        };
        let locked = config.lock(&ctx).unwrap();
        let test_dir = ctx.clone_dir.join("github.com/rossmacarthur/another-dir");
        let test_file = test_dir.join("test.txt");
        fs::create_dir_all(&test_dir).unwrap();
        {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&test_file)
                .unwrap();
        }

        assert_eq!(locked.clean(&ctx).len(), 0);
        assert!(ctx
            .clone_dir
            .join("github.com/rossmacarthur/sheldon-test")
            .exists());
        assert!(ctx
            .clone_dir
            .join("github.com/rossmacarthur/sheldon-test/test.plugin.zsh")
            .exists());
        assert!(!test_file.exists());
        assert!(!test_dir.exists());
    }

    #[test]
    fn locked_config_to_and_from_path() {
        let mut temp = tempfile::NamedTempFile::new().unwrap();
        let content = r#"version = "<version>"
home = "<root>"
root = "<root>"
config_file = "<root>/plugins.toml"
lock_file = "<root>/plugins.lock"
clone_dir = "<root>/repositories"
download_dir = "<root>/downloads"
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
