//! Plugin installation.
//!
//! This module handles the downloading of `Source`s and figuring out which
//! filenames to use for `Plugins`.

use std::{
    cmp,
    collections::HashMap,
    fmt, fs, io,
    path::{Path, PathBuf},
    result, sync,
};

use indexmap::{indexmap, IndexMap};
use itertools::Itertools;
use lazy_static::lazy_static;
use maplit::hashmap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    config::{Config, GitReference, Plugin, Source, Template},
    context::Context,
    Result, ResultExt,
};

/// The default clone directory for `Git` sources.
const CLONE_DIRECTORY: &str = "repositories";

/// The default download directory for `Remote` sources.
const DOWNLOAD_DIRECTORY: &str = "downloads";

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

/// A locked `Plugin`.
#[derive(Debug, Deserialize, Serialize)]
struct LockedPlugin {
    /// The name of this plugin.
    name: String,
    /// The directory that this plugin resides in.
    directory: PathBuf,
    /// The filenames to use in the directory.
    filenames: Vec<PathBuf>,
    /// What templates to apply to each filename.
    apply: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
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
}

/////////////////////////////////////////////////////////////////////////
// Lock implementations.
/////////////////////////////////////////////////////////////////////////

impl PartialEq<LockedContext> for Context {
    fn eq(&self, other: &LockedContext) -> bool {
        self.version == other.version
            && self.home == other.home
            && self.root == other.root
            && self.config_file == other.config_file
            && self.lock_file == other.lock_file
    }
}

impl GitReference {
    /// Consume the `GitReference` and convert it to a `LockedGitReference`.
    ///
    /// This code is take from [Cargo].
    ///
    /// [Cargo]: https://github.com/rust-lang/cargo/blob/master/src/cargo/sources/git/utils.rs#L207
    fn lock(&self, repo: &git2::Repository) -> Result<LockedGitReference> {
        let reference = match self {
            GitReference::Branch(s) => repo
                .find_branch(&format!("origin/{}", s), git2::BranchType::Remote)
                .chain(s!("failed to find branch `{}`", s))?
                .get()
                .target()
                .chain(s!("branch `{}` does not have a target", s))?,
            GitReference::Revision(s) => {
                let obj = repo
                    .revparse_single(s)
                    .chain(s!("failed to find revision `{}`", s))?;
                match obj.as_tag() {
                    Some(tag) => tag.target_id(),
                    None => obj.id(),
                }
            }
            GitReference::Tag(s) => (|| -> result::Result<_, git2::Error> {
                let id = repo.refname_to_id(&format!("refs/tags/{}", s))?;
                let obj = repo.find_object(id, None)?;
                let obj = obj.peel(git2::ObjectType::Commit)?;
                Ok(obj.id())
            })()
            .chain(s!("failed to find tag `{}`", s))?,
        };
        Ok(LockedGitReference(reference))
    }
}

impl fmt::Display for GitReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GitReference::Branch(s) | GitReference::Revision(s) | GitReference::Tag(s) => {
                write!(f, "{}", s)
            }
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Source::Git {
                url,
                reference: Some(reference),
            } => write!(f, "{}@{}", url, reference),
            Source::Git { url, .. } | Source::Remote { url } => write!(f, "{}", url),
            Source::Local { directory } => write!(f, "{}", directory.display()),
        }
    }
}

impl Source {
    /// Clone a Git repository and checks it out at a particular revision.
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

        let mut cloned = false;

        // Clone or open the repository.
        let repo = match git2::Repository::clone(&url.to_string(), &directory) {
            Ok(repo) => {
                cloned = true;
                repo
            }
            Err(e) => {
                if e.code() != git2::ErrorCode::Exists {
                    return Err(e).chain(s!("failed to git clone `{}`", url));
                } else {
                    git2::Repository::open(&directory)
                        .chain(s!("failed to open repository at `{}`", directory.display()))?
                }
            }
        };

        let status = if cloned { "Cloned" } else { "Checked" };

        // Checkout the configured revision.
        if let Some(reference) = reference {
            let revision = reference.lock(&repo)?;

            let obj = repo
                .find_object(revision.0, None)
                .chain(s!("failed to find revision `{}`", revision.0))?;
            repo.reset(&obj, git2::ResetType::Hard, None).chain(s!(
                "failed to reset repository to revision `{}`",
                revision.0
            ))?;

            ctx.status(status, &format!("{}@{}", &url, reference));
        } else {
            ctx.status(status, &url);
        }

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

        if fs::metadata(&directory)
            .chain(s!("failed to find directory `{}`", directory.display()))?
            .is_dir()
        {
            ctx.status("Checked", &ctx.replace_home(&directory).display());
            Ok(LockedSource {
                directory,
                filename: None,
            })
        } else {
            bail!("`{}` is not a directory", directory.display());
        }
    }

    /// Install this `Source`.
    fn lock(self, ctx: &Context) -> Result<LockedSource> {
        match self {
            Source::Git { url, reference } => {
                let mut directory = ctx.root.join(CLONE_DIRECTORY);
                directory.push(url.host_str().chain(s!("URL `{}` has no host", url))?);
                directory.push(url.path().trim_start_matches('/'));
                Self::lock_git(ctx, directory, url, reference)
            }
            Source::Remote { url } => {
                let mut directory = ctx.root.join(DOWNLOAD_DIRECTORY);
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
            Source::Local { directory } => Self::lock_local(ctx, directory),
        }
    }
}

impl Plugin {
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

    /// Consume the `Plugin` and convert it to a `LockedPlugin`.
    fn lock(
        self,
        ctx: &Context,
        source: LockedSource,
        matches: &[String],
        apply: &[String],
    ) -> Result<LockedPlugin> {
        Ok(if let Source::Remote { .. } = self.source {
            let LockedSource {
                directory,
                filename,
            } = source;
            LockedPlugin {
                name: self.name,
                directory,
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
                    .chain(s!("root directory is not valid UTF-8"))?,
                "name" => &self.name
            };

            let directory = if let Some(directory) = self.directory {
                let rendered = templates
                    .render_template(&directory, &data)
                    .chain(s!("failed to render template `{}`", directory))?;
                source.directory.join(rendered)
            } else {
                source.directory
            };
            data.insert(
                "directory",
                &directory
                    .to_str()
                    .chain(s!("directory is not valid UTF-8"))?,
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

            LockedPlugin {
                name: self.name,
                directory,
                filenames,
                apply: self.apply.unwrap_or_else(|| apply.to_vec()),
            }
        })
    }
}

impl Context {
    /// Consume the `Context` and convert it to a `LockedContext`.
    fn lock(self) -> LockedContext {
        LockedContext {
            version: self.version.to_string(),
            home: self.home,
            root: self.root,
            config_file: self.config_file,
            lock_file: self.lock_file,
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
        // Create a map of unique `Source` to `Vec<Plugin>`
        let mut map = IndexMap::new();
        for (index, plugin) in self.plugins.into_iter().enumerate() {
            map.entry(plugin.source.clone())
                .or_insert_with(|| Vec::with_capacity(1))
                .push((index, plugin));
        }

        let matches = &self.matches;
        let apply = &self.apply.as_ref().unwrap_or(&*DEFAULT_APPLY);
        let count = map.len();

        let plugins = if count == 0 {
            Vec::new()
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
                                        .chain(s!("failed to install plugin `{}`", name))?,
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
                .take(count)
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .into_iter()
                .sorted_by_key(|(index, _)| *index)
                .map(|(_, plugin)| plugin)
                .collect()
        };

        Ok(LockedConfig {
            ctx: ctx.clone().lock(),
            templates: self.templates,
            plugins,
        })
    }
}

impl LockedConfig {
    /// Read a `LockedConfig` from the given path.
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();;
        let locked: LockedConfig = toml::from_str(&String::from_utf8_lossy(
            &fs::read(&path).chain(s!("failed to read locked config from `{}`", path.display()))?,
        ))
        .chain(s!("failed to deserialize locked config"))?;
        Ok(locked)
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
            for name in &plugin.apply {
                // Data to use in template rendering
                let mut data = hashmap! {
                    "root" => self
                        .ctx.root
                        .to_str()
                        .chain(s!("root directory is not valid UTF-8"))?,
                    "name" => &plugin.name,
                    "directory" => plugin
                        .directory
                        .to_str()
                        .chain(s!("root directory is not valid UTF-8"))?,
                };

                if templates_map.get(name.as_str()).unwrap().each {
                    for filename in &plugin.filenames {
                        data.insert(
                            "filename",
                            filename.to_str().chain(s!("filename is not valid UTF-8"))?,
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
            &toml::to_string(&self).chain(s!("failed to serialize locked config"))?,
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
    use std::{fs, io::Read, process::Command};
    use url::Url;

    fn git_create_test_repo(directory: &Path) {
        Command::new("git")
            .arg("-C")
            .arg(&directory)
            .arg("init")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&directory)
            .arg("remote")
            .arg("add")
            .arg("origin")
            .arg("https://github.com/rossmacarthur/sheldon-test")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&directory)
            .arg("fetch")
            .output()
            .unwrap();
        Command::new("touch")
            .arg(directory.join("test.txt"))
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("add")
            .arg(".")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("commit")
            .arg("-m")
            .arg("Initial commit")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("tag")
            .arg("derp")
            .output()
            .unwrap();
    }

    fn git_get_last_commit(directory: &Path) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("log")
            .arg("-n")
            .arg("1")
            .arg("--pretty=format:\"%H\"")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().trim_matches('"').to_string()
    }

    fn git_get_last_origin_commit(directory: &Path) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("log")
            .arg("-n")
            .arg("1")
            .arg("--pretty=format:\"%H\"")
            .arg("origin/master")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().trim_matches('"').to_string()
    }

    fn git_status(directory: &Path) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("status")
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn read_file_contents(filename: &Path) -> result::Result<String, io::Error> {
        let mut file = fs::File::open(filename)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    #[test]
    fn git_reference_lock_tag() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();
        git_create_test_repo(&directory);
        let hash = git_get_last_commit(&directory);
        let repo = git2::Repository::open(directory).unwrap();

        let reference = GitReference::Tag("derp".to_string());
        let locked = reference.lock(&repo).unwrap();

        assert_eq!(locked.0.to_string(), hash);
    }

    #[test]
    fn git_reference_lock_branch() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();
        git_create_test_repo(&directory);
        let hash = git_get_last_origin_commit(&directory);
        let repo = git2::Repository::open(directory).unwrap();

        let reference = GitReference::Branch("master".to_string());
        let locked = reference.lock(&repo).unwrap();

        assert_eq!(locked.0.to_string(), hash);
    }

    #[test]
    fn git_reference_lock_revision() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();
        git_create_test_repo(&directory);
        let hash = git_get_last_commit(&directory);
        let repo = git2::Repository::open(directory).unwrap();

        let reference = GitReference::Revision(hash.clone());
        let locked = reference.lock(&repo).unwrap();

        assert_eq!(locked.0.to_string(), hash);
    }

    #[test]
    fn source_lock_git() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();

        let locked = Source::lock_git(
            &create_test_context(&directory.to_string_lossy()),
            directory.to_path_buf(),
            Url::parse("https://github.com/rossmacarthur/sheldon").unwrap(),
            None,
        )
        .unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
        assert_eq!(
            git_status(&directory),
            "On branch master\nYour branch is up to date with 'origin/master'.\n\nnothing to \
             commit, working tree clean\n"
        );
    }

    #[test]
    fn source_lock_git_with_reference() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();

        let locked = Source::lock_git(
            &create_test_context(&directory.to_string_lossy()),
            directory.to_path_buf(),
            Url::parse("https://github.com/rossmacarthur/sheldon").unwrap(),
            Some(GitReference::Tag("0.2.0".to_string())),
        )
        .unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, None);
        assert_eq!(
            git_get_last_commit(&directory),
            "a2cf341b37c958e490aafc92dd775c597addf3c4"
        );
    }

    #[test]
    fn source_lock_remote() {
        let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();
        let filename = directory.join("test.txt");

        let locked = Source::lock_remote(
            &create_test_context(&directory.to_string_lossy()),
            directory.to_path_buf(),
            filename.clone(),
            Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT").unwrap(),
        )
        .unwrap();

        assert_eq!(locked.directory, directory);
        assert_eq!(locked.filename, Some(filename.clone()));
        assert_eq!(
            read_file_contents(&filename).unwrap(),
            read_file_contents(&manifest_dir.join("LICENSE-MIT")).unwrap()
        )
    }

    fn create_test_context(root: &str) -> Context {
        let root = PathBuf::from(root);
        Context {
            verbosity: crate::Verbosity::Quiet,
            no_color: true,
            version: clap::crate_version!(),
            home: "/".into(),
            config_file: root.join("config.toml"),
            lock_file: root.join("config.lock"),
            root,
            reinstall: false,
            relock: false,
        }
    }

    #[test]
    fn plugin_lock_git_with_uses() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let directory = root.join("repositories/github.com/rossmacarthur/sheldon");
        fs::create_dir_all(&directory).unwrap();
        fs::File::create(directory.join("1.txt")).unwrap();
        fs::File::create(directory.join("2.txt")).unwrap();
        fs::File::create(directory.join("test.html")).unwrap();

        let plugin = Plugin {
            name: "test".into(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon").unwrap(),
                reference: None,
            },
            directory: None,
            uses: Some(vec!["*.txt".into(), "{{ name }}.html".into()]),
            apply: None,
        };
        let locked = plugin
            .lock(
                &create_test_context(&root.to_string_lossy()),
                LockedSource {
                    directory: directory.clone(),
                    filename: None,
                },
                &[],
                &["hello".into()],
            )
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.directory, directory);
        assert_eq!(
            locked.filenames,
            vec![
                directory.join("1.txt"),
                directory.join("2.txt"),
                directory.join("test.html")
            ]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn plugin_lock_git_with_matches() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let directory = root.join("repositories/github.com/rossmacarthur/sheldon");
        fs::create_dir_all(&directory).unwrap();
        fs::File::create(directory.join("1.txt")).unwrap();
        fs::File::create(directory.join("2.txt")).unwrap();
        fs::File::create(directory.join("test.html")).unwrap();

        let plugin = Plugin {
            name: "test".into(),
            source: Source::Git {
                url: Url::parse("https://github.com/rossmacarthur/sheldon").unwrap(),
                reference: None,
            },
            directory: None,
            uses: None,
            apply: None,
        };
        let locked = plugin
            .lock(
                &create_test_context(&root.to_string_lossy()),
                LockedSource {
                    directory: directory.clone(),
                    filename: None,
                },
                &["*.txt".into(), "test.html".into()],
                &["hello".into()],
            )
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(locked.directory, directory);
        assert_eq!(
            locked.filenames,
            vec![directory.join("1.txt"), directory.join("2.txt")]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn plugin_lock_remote() {
        let plugin = Plugin {
            name: "test".into(),
            source: Source::Remote {
                url: Url::parse("https://ross.macarthur.io/test.html").unwrap(),
            },
            directory: None,
            uses: None,
            apply: None,
        };
        let locked = plugin
            .lock(
                &create_test_context("/home/test"),
                LockedSource {
                    directory: "/home/test/downloads/ross.macarthur.io".into(),
                    filename: Some("/home/test/downloads/ross.macarthur.io/test.html".into()),
                },
                &[],
                &["hello".into()],
            )
            .unwrap();

        assert_eq!(locked.name, String::from("test"));
        assert_eq!(
            locked.directory,
            PathBuf::from("/home/test/downloads/ross.macarthur.io")
        );
        assert_eq!(
            locked.filenames,
            vec![PathBuf::from(
                "/home/test/downloads/ross.macarthur.io/test.html"
            )]
        );
        assert_eq!(locked.apply, vec![String::from("hello")]);
    }

    #[test]
    fn config_lock_example_config() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let ctx = Context {
            verbosity: crate::Verbosity::Quiet,
            no_color: true,
            version: clap::crate_version!(),
            home: "/".into(),
            root: root.to_path_buf(),
            config_file: manifest_dir.join("docs/plugins.example.toml"),
            lock_file: root.join("plugins.lock"),
            reinstall: false,
            relock: false,
        };
        let pyenv_dir = root.join("pyenv");
        fs::create_dir(&pyenv_dir).unwrap();

        let mut config = Config::from_path(&ctx.config_file).unwrap();
        config.plugins[2].source = Source::Local {
            directory: pyenv_dir,
        };
        config.lock(&ctx).unwrap();
    }
}
