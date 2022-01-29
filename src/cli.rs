//! Command line interface.

use std::env;
use std::fmt;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;

use anyhow::anyhow;
use clap::{AppSettings, ArgGroup, Parser};
use thiserror::Error;
use url::Url;

use crate::build;
use crate::config::{
    GistRepository, GitHubRepository, GitProtocol, GitReference, RawPlugin, Shell,
};
use crate::context::{LockMode, Settings};
use crate::edit::Plugin;
use crate::log::{Output, Verbosity};

/// Whether messages should use color output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorChoice {
    /// Force color output.
    Always,
    /// Intelligently guess whether to use color output.
    Auto,
    /// Force disable color output.
    Never,
}

#[derive(Debug, PartialEq, Parser)]
#[clap(
    group = ArgGroup::new("plugin").required(true),
    group = ArgGroup::new("git-reference").conflicts_with_all(&["remote", "local"]),
)]
struct Add {
    /// A unique name for this plugin.
    #[clap(value_name = "NAME")]
    name: String,

    /// Add a clonable Git repository.
    #[clap(long, value_name = "URL", group = "plugin")]
    git: Option<Url>,

    /// Add a clonable Gist snippet.
    #[clap(long, value_name = "ID", group = "plugin")]
    gist: Option<GistRepository>,

    /// Add a clonable GitHub repository.
    #[clap(long, value_name = "REPO", group = "plugin")]
    github: Option<GitHubRepository>,

    /// Add a downloadable file.
    #[clap(long, value_name = "URL", group = "plugin")]
    remote: Option<Url>,

    /// Add a local directory.
    #[clap(long, value_name = "DIR", group = "plugin")]
    local: Option<PathBuf>,

    /// The Git protocol for a Gist or GitHub plugin.
    #[clap(long, value_name = "PROTO", conflicts_with_all = &["git", "remote", "local"])]
    proto: Option<GitProtocol>,

    /// Checkout the tip of a branch.
    #[clap(long, value_name = "BRANCH", group = "git-reference")]
    branch: Option<String>,

    /// Checkout a specific commit.
    #[clap(long, value_name = "SHA", group = "git-reference")]
    rev: Option<String>,

    /// Checkout a specific tag.
    #[clap(long, value_name = "TAG", group = "git-reference")]
    tag: Option<String>,

    /// Which sub directory to use in this plugin.
    #[clap(long, value_name = "PATH")]
    dir: Option<String>,

    /// Which files to use in this plugin.
    #[clap(long = "use", value_name = "MATCH", multiple_values(true))]
    uses: Option<Vec<String>>,

    /// Templates to apply to this plugin.
    #[clap(long, value_name = "TEMPLATE", multiple_values(true))]
    apply: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Parser)]
enum RawCommand {
    /// Initialize a new config file.
    Init {
        /// The type of shell, accepted values are: bash, zsh.
        #[clap(long, value_name = "SHELL")]
        shell: Option<Shell>,
    },

    /// Add a new plugin to the config file.
    Add(Box<Add>),

    /// Open up the config file in the default editor.
    Edit,

    /// Remove a plugin from the config file.
    Remove {
        /// A unique name for this plugin.
        #[clap(value_name = "NAME")]
        name: String,
    },

    /// Install the plugins sources and generate the lock file.
    Lock {
        /// Update all plugin sources.
        #[clap(long)]
        update: bool,

        /// Reinstall all plugin sources.
        #[clap(long, conflicts_with = "update")]
        reinstall: bool,
    },

    /// Generate and print out the script.
    Source {
        /// Regenerate the lock file.
        #[clap(long)]
        relock: bool,

        /// Update all plugin sources (implies --relock).
        #[clap(long)]
        update: bool,

        /// Reinstall all plugin sources (implies --relock).
        #[clap(long, conflicts_with = "update")]
        reinstall: bool,
    },

    /// Prints detailed version information.
    Version,
}

#[derive(Debug, PartialEq, Parser)]
#[clap(
    author,
    about,
    version = build::CRATE_RELEASE,
    long_version = build::CRATE_LONG_VERSION.as_str(),
    term_width = 120,
    global_setting = AppSettings::DeriveDisplayOrder,
    global_setting = AppSettings::DisableHelpSubcommand,
    global_setting = AppSettings::DisableColoredHelp,
    global_setting = AppSettings::PropagateVersion,
    setting = AppSettings::SubcommandRequired,
)]
struct RawOpt {
    /// Suppress any informational output.
    #[clap(long, short)]
    quiet: bool,

    /// Use verbose output.
    #[clap(long, short)]
    verbose: bool,

    /// Output coloring: always, auto, or never.
    #[clap(long, value_name = "WHEN", default_value_t)]
    color: ColorChoice,

    /// The home directory.
    #[clap(long, value_name = "PATH", hide(true))]
    home: Option<PathBuf>,

    /// The configuration directory.
    #[clap(long, value_name = "PATH", env = "SHELDON_CONFIG_DIR")]
    config_dir: Option<PathBuf>,

    /// The data directory
    #[clap(long, value_name = "PATH", env = "SHELDON_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// The config file.
    #[clap(long, value_name = "PATH", env = "SHELDON_CONFIG_FILE")]
    config_file: Option<PathBuf>,

    /// The lock file.
    #[clap(long, value_name = "PATH", env = "SHELDON_LOCK_FILE")]
    lock_file: Option<PathBuf>,

    /// The directory where git sources are cloned to.
    #[clap(long, value_name = "PATH", env = "SHELDON_CLONE_DIR")]
    clone_dir: Option<PathBuf>,

    /// The directory where remote sources are downloaded to.
    #[clap(long, value_name = "PATH", env = "SHELDON_DOWNLOAD_DIR")]
    download_dir: Option<PathBuf>,

    /// The subcommand to run.
    #[clap(subcommand)]
    command: RawCommand,
}

/// The resolved command.
#[derive(Debug)]
pub enum Command {
    /// Initialize a new config file.
    Init { shell: Option<Shell> },
    /// Add a new plugin to the config file.
    Add { name: String, plugin: Box<Plugin> },
    /// Open up the config file in the default editor.
    Edit,
    /// Remove a plugin from the config file.
    Remove { name: String },
    /// Install the plugins sources and generate the lock file.
    Lock { mode: LockMode },
    /// Generate and print out the script.
    Source { relock: bool, mode: LockMode },
}

/// Resolved command line options with defaults set.
#[derive(Debug)]
pub struct Opt {
    /// Global settings for use across the entire program.
    pub settings: Settings,
    /// The output style.
    pub output: Output,
    /// The subcommand.
    pub command: Command,
}

impl Default for ColorChoice {
    fn default() -> Self {
        Self::Auto
    }
}

impl fmt::Display for ColorChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => f.write_str("always"),
            Self::Auto => f.write_str("auto"),
            Self::Never => f.write_str("never"),
        }
    }
}

#[derive(Debug, Error)]
#[error("expected `always` or `never`, got `{}`", self.0)]
pub struct ParseColorChoiceError(String);

impl FromStr for ColorChoice {
    type Err = ParseColorChoiceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "always" => Ok(Self::Always),
            "auto" => Ok(Self::Auto),
            "never" => Ok(Self::Never),
            s => Err(ParseColorChoiceError(s.to_string())),
        }
    }
}

impl ColorChoice {
    fn is_no_color(self) -> bool {
        match self {
            Self::Always => false,
            Self::Auto => !atty::is(atty::Stream::Stderr),
            Self::Never => true,
        }
    }
}

impl LockMode {
    fn from_lock_flags(update: bool, reinstall: bool) -> Self {
        match (update, reinstall) {
            (false, false) => Self::Normal,
            (true, false) => Self::Update,
            (false, true) => Self::Reinstall,
            (true, true) => unreachable!(),
        }
    }

    fn from_source_flags(relock: bool, update: bool, reinstall: bool) -> (bool, Self) {
        match (relock, update, reinstall) {
            (relock, false, false) => (relock, Self::Normal),
            (_, true, false) => (true, Self::Update),
            (_, false, true) => (true, Self::Reinstall),
            (_, true, true) => unreachable!(),
        }
    }
}

impl Plugin {
    fn from_add(add: Add) -> (String, Self) {
        let Add {
            name,
            git,
            gist,
            github,
            remote,
            local,
            proto,
            branch,
            rev,
            tag,
            dir,
            uses,
            apply,
        } = add;

        let reference = match (branch, rev, tag) {
            (Some(s), None, None) => Some(GitReference::Branch(s)),
            (None, Some(s), None) => Some(GitReference::Rev(s)),
            (None, None, Some(s)) => Some(GitReference::Tag(s)),
            (None, None, None) => None,
            // this is unreachable because these three options are in the same mutually exclusive
            // 'git-reference' CLI group
            _ => unreachable!(),
        };

        (
            name,
            Self::from(RawPlugin {
                git,
                gist,
                github,
                remote,
                local,
                inline: None,
                proto,
                reference,
                dir,
                uses,
                apply,
                rest: None,
            }),
        )
    }
}

impl Opt {
    fn from_raw_opt(raw_opt: RawOpt) -> Self {
        let RawOpt {
            quiet,
            verbose,
            color,
            home,
            data_dir,
            config_dir,
            config_file,
            lock_file,
            clone_dir,
            download_dir,
            command,
        } = raw_opt;

        let command = match command {
            RawCommand::Init { shell } => Command::Init { shell },
            RawCommand::Add(add) => {
                let (name, plugin) = Plugin::from_add(*add);
                Command::Add {
                    name,
                    plugin: Box::new(plugin),
                }
            }
            RawCommand::Edit => Command::Edit,
            RawCommand::Remove { name } => Command::Remove { name },
            RawCommand::Lock { update, reinstall } => {
                let mode = LockMode::from_lock_flags(update, reinstall);
                Command::Lock { mode }
            }
            RawCommand::Source {
                relock,
                update,
                reinstall,
            } => {
                let (relock, mode) = LockMode::from_source_flags(relock, update, reinstall);
                Command::Source { relock, mode }
            }
            RawCommand::Version => {
                println!("{} {}", build::CRATE_NAME, &*build::CRATE_VERBOSE_VERSION);
                process::exit(0);
            }
        };

        let verbosity = if quiet {
            Verbosity::Quiet
        } else if verbose {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        };

        let output = Output {
            verbosity,
            no_color: color.is_no_color(),
        };

        let home = match home.or_else(home::home_dir).ok_or_else(|| {
            anyhow!(
                "failed to determine the current user's home directory, try using the `--home` \
                 option"
            )
        }) {
            Ok(home) => home,
            Err(err) => {
                error!(&output, &err);
                process::exit(1);
            }
        };

        let xdg_config_user = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let xdg_data_user = env::var_os("XDG_DATA_HOME").map(PathBuf::from);

        // Note: `XDG_RUNTIME_DIR` is not checked as it can be set by the system rather
        // than the user, and cannot be relied upon to indicate a preference for XDG
        // directory layout.
        let using_xdg = any!(
            xdg_data_user,
            xdg_config_user,
            env::var_os("XDG_CACHE_HOME"),
            env::var_os("XDG_DATA_DIRS"),
            env::var_os("XDG_CONFIG_DIRS")
        );

        let (config_pre, data_pre) = if using_xdg {
            (
                xdg_config_user
                    .unwrap_or_else(|| home.join(".config"))
                    .join("sheldon"),
                xdg_data_user
                    .unwrap_or_else(|| home.join(".local/share"))
                    .join("sheldon"),
            )
        } else {
            (home.join(".sheldon"), home.join(".sheldon"))
        };

        let config_dir = config_dir.unwrap_or(config_pre);
        let data_dir = data_dir.unwrap_or(data_pre);
        let config_file = config_file.unwrap_or_else(|| config_dir.join("plugins.toml"));
        let lock_file = lock_file.unwrap_or_else(|| data_dir.join("plugins.lock"));
        let clone_dir = clone_dir.unwrap_or_else(|| data_dir.join("repos"));
        let download_dir = download_dir.unwrap_or_else(|| data_dir.join("downloads"));

        let settings = Settings {
            version: build::CRATE_RELEASE.to_string(),
            home,
            config_dir,
            data_dir,
            config_file,
            lock_file,
            clone_dir,
            download_dir,
        };

        Self {
            settings,
            output,
            command,
        }
    }

    /// Gets the struct from the command line arguments. Print the error message
    /// and quit the program in case of failure.
    pub fn from_args() -> Self {
        Self::from_raw_opt(RawOpt::parse())
    }
}

////////////////////////////////////////////////////////////////////////////////
// Unit tests
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use std::iter;

    use clap::{crate_authors, crate_description, crate_name};
    use pretty_assertions::assert_eq;
    use serde_json as json;
    use serde_json::json;

    fn setup() {
        for (k, _) in env::vars() {
            if k.starts_with(&format!("{}_", crate_name!().to_uppercase())) || k.starts_with("XDG_")
            {
                env::remove_var(k);
            }
        }
    }

    fn ctx() -> json::Value {
        json!({
            "name": build::CRATE_NAME,
            "version": build::CRATE_RELEASE,
            "authors": crate_authors!(),
            "description": crate_description!(),
        })
    }

    fn raw_opt(args: &[&str]) -> RawOpt {
        RawOpt::try_parse_from(iter::once(crate_name!()).chain(args.iter().copied())).unwrap()
    }

    fn raw_opt_err(args: &[&str]) -> clap::Error {
        RawOpt::try_parse_from(iter::once(crate_name!()).chain(args.iter().copied())).unwrap_err()
    }

    #[test]
    fn raw_opt_version() {
        setup();
        let err = raw_opt_err(&["-V"]);
        assert_eq!(
            err.to_string(),
            format!("sheldon {}\n", &*build::CRATE_RELEASE)
        );
        assert_eq!(err.kind, clap::ErrorKind::DisplayVersion);
    }

    #[test]
    fn raw_opt_long_version() {
        setup();
        let err = raw_opt_err(&["--version"]);
        assert_eq!(
            err.to_string(),
            format!("sheldon {}\n", &*build::CRATE_LONG_VERSION)
        );
        assert_eq!(err.kind, clap::ErrorKind::DisplayVersion);
    }

    #[test]
    fn raw_opt_help() {
        setup();
        for opt in &["-h", "--help"] {
            let err = raw_opt_err(&[opt]);
            goldie::assert_template!(ctx(), err.to_string());
            assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
        }
    }

    #[test]
    fn raw_opt_no_options() {
        setup();
        assert_eq!(
            raw_opt(&["lock"]),
            RawOpt {
                quiet: false,
                verbose: false,
                color: Default::default(),
                home: None,
                config_dir: None,
                data_dir: None,
                config_file: None,
                lock_file: None,
                clone_dir: None,
                download_dir: None,
                command: RawCommand::Lock {
                    update: false,
                    reinstall: false
                },
            }
        );
    }

    #[test]
    fn raw_opt_options() {
        setup();
        assert_eq!(
            raw_opt(&[
                "--quiet",
                "--verbose",
                "--color",
                "never",
                "--home",
                "/",
                "--config-dir",
                "/test",
                "--data-dir",
                "/test",
                "--config-file",
                "/plugins.toml",
                "--lock-file",
                "/test/plugins.lock",
                "--clone-dir",
                "/repos",
                "--download-dir",
                "/downloads",
                "lock",
            ]),
            RawOpt {
                quiet: true,
                verbose: true,
                color: ColorChoice::Never,
                home: Some("/".into()),
                config_dir: Some("/test".into()),
                data_dir: Some("/test".into()),
                config_file: Some("/plugins.toml".into()),
                lock_file: Some("/test/plugins.lock".into()),
                clone_dir: Some("/repos".into()),
                download_dir: Some("/downloads".into()),
                command: RawCommand::Lock {
                    update: false,
                    reinstall: false
                },
            }
        );
    }

    #[test]
    fn raw_opt_subcommand_required() {
        setup();
        let err = raw_opt_err(&[]);
        goldie::assert_template!(ctx(), err.to_string());
        assert_eq!(err.kind, clap::ErrorKind::MissingSubcommand);
    }

    #[test]
    fn raw_opt_init_help() {
        setup();
        let err = raw_opt_err(&["init", "--help"]);
        goldie::assert_template!(ctx(), err.to_string());
        assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
    }

    #[test]
    fn raw_opt_init_with_invalid_shell() {
        setup();
        assert_eq!(
            raw_opt_err(&["init", "--shell", "ksh",]).kind,
            clap::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn raw_opt_add_help() {
        setup();
        let err = raw_opt_err(&["add", "--help"]);
        goldie::assert_template!(ctx(), err.to_string());
        assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
    }

    #[test]
    fn raw_opt_add_no_source() {
        setup();
        assert_eq!(
            raw_opt_err(&["add", "test",]).kind,
            clap::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn raw_opt_add_git_with_options() {
        setup();
        assert_eq!(
            raw_opt(&[
                "add",
                "test",
                "--git",
                "https://github.com/rossmacarthur/sheldon-test",
                "--rev",
                "ad149784a1538291f2477fb774eeeed4f4d29e45",
                "--dir",
                "missing",
                "--use",
                "{name}.sh",
                "*.zsh",
                "--apply",
                "something",
                "another-thing"
            ])
            .command,
            RawCommand::Add(Box::new(Add {
                name: "test".to_string(),
                git: Some(
                    "https://github.com/rossmacarthur/sheldon-test"
                        .parse()
                        .unwrap()
                ),
                gist: None,
                github: None,
                remote: None,
                local: None,
                proto: None,
                branch: None,
                rev: Some("ad149784a1538291f2477fb774eeeed4f4d29e45".into()),
                tag: None,
                dir: Some("missing".into()),
                uses: Some(vec_into!["{name}.sh", "*.zsh"]),
                apply: Some(vec_into!["something", "another-thing"]),
            }))
        );
    }

    #[test]
    fn raw_opt_add_gist_options() {
        setup();
        assert_eq!(
            raw_opt(&[
                "add",
                "test",
                "--gist",
                "579d02802b1cc17baed07753d09f5009",
                "--tag",
                "0.1.0",
                "--proto",
                "ssh",
                "--dir",
                "missing",
                "--use",
                "{name}.sh",
                "*.zsh",
                "--apply",
                "something",
                "another-thing"
            ])
            .command,
            RawCommand::Add(Box::new(Add {
                name: "test".to_string(),
                git: None,
                gist: Some("579d02802b1cc17baed07753d09f5009".parse().unwrap()),
                github: None,
                remote: None,
                local: None,
                proto: Some("ssh".parse().unwrap()),
                branch: None,
                rev: None,
                tag: Some("0.1.0".into()),
                dir: Some("missing".into()),
                uses: Some(vec_into!["{name}.sh", "*.zsh"]),
                apply: Some(vec_into!["something", "another-thing"]),
            }))
        );
    }

    #[test]
    fn raw_opt_add_github_options() {
        setup();
        assert_eq!(
            raw_opt(&[
                "add",
                "test",
                "--github",
                "rossmacarthur/sheldon-test",
                "--branch",
                "feature",
                "--proto",
                "https",
                "--dir",
                "missing",
                "--use",
                "{name}.sh",
                "*.zsh",
                "--apply",
                "something",
                "another-thing"
            ])
            .command,
            RawCommand::Add(Box::new(Add {
                name: "test".to_string(),
                git: None,
                gist: None,
                github: Some("rossmacarthur/sheldon-test".parse().unwrap()),
                remote: None,
                local: None,
                proto: Some("https".parse().unwrap()),
                branch: Some("feature".into()),
                rev: None,
                tag: None,
                dir: Some("missing".into()),
                uses: Some(vec_into!["{name}.sh", "*.zsh"]),
                apply: Some(vec_into!["something", "another-thing"]),
            }))
        );
    }

    #[test]
    fn raw_opt_add_remote_options() {
        setup();
        assert_eq!(
            raw_opt(&[
                "add",
                "test",
                "--remote",
                "https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh",
                "--use",
                "{name}.sh",
                "*.zsh",
                "--apply",
                "something",
                "another-thing"
            ])
            .command,
            RawCommand::Add(Box::new(Add {
                name: "test".to_string(),
                git: None,
                gist: None,
                github: None,
                remote: Some("https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh".parse().unwrap()),
                local: None,
                proto: None,
                branch: None,
                rev: None,
                tag: None,
                dir: None,
                uses: Some(vec_into!["{name}.sh", "*.zsh"]),
                apply: Some(vec_into!["something", "another-thing"]),
            }))
        );
    }

    #[test]
    fn raw_opt_add_local_options() {
        setup();
        assert_eq!(
            raw_opt(&[
                "add",
                "test",
                "--local",
                "~/.dotfiles/zsh/pure",
                "--use",
                "{name}.sh",
                "*.zsh",
                "--apply",
                "something",
                "another-thing"
            ])
            .command,
            RawCommand::Add(Box::new(Add {
                name: "test".to_string(),
                git: None,
                gist: None,
                github: None,
                remote: None,
                local: Some("~/.dotfiles/zsh/pure".into()),
                proto: None,
                branch: None,
                rev: None,
                tag: None,
                dir: None,
                uses: Some(vec_into!["{name}.sh", "*.zsh"]),
                apply: Some(vec_into!["something", "another-thing"]),
            }))
        );
    }

    #[test]
    fn raw_opt_add_remote_with_reference_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&[
                "add",
                "test",
                "--remote",
                "https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh",
                "--branch",
                "feature"
            ])
            .kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_add_local_with_reference_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&[
                "add",
                "test",
                "--local",
                "~/.dotfiles/zsh/pure",
                "--tag",
                "0.1.0"
            ])
            .kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_add_git_with_github_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&[
                "add",
                "test",
                "--git",
                "https://github.com/rossmacarthur/sheldon-test",
                "--github",
                "rossmacarthur/sheldon-test",
            ])
            .kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_add_git_with_protocol_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&[
                "add",
                "test",
                "--git",
                "https://github.com/rossmacarthur/sheldon-test",
                "--proto",
                "ssh",
            ])
            .kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_add_remote_with_protocol_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&[
                "add",
                "test",
                "--remote",
                "https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh",
                "--proto",
                "ssh",
            ])
            .kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_add_local_with_protocol_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&[
                "add",
                "test",
                "--local",
                "~/.dotfiles/zsh/pure",
                "--proto",
                "ssh",
            ])
            .kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_lock_help() {
        setup();
        let err = raw_opt_err(&["lock", "--help"]);
        goldie::assert_template!(ctx(), err.to_string());
        assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
    }

    #[test]
    fn raw_opt_lock_with_update_and_reinstall_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&["lock", "--update", "--reinstall"]).kind,
            clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_source_help() {
        setup();
        let err = raw_opt_err(&["source", "--help"]);
        goldie::assert_template!(ctx(), err.to_string());
        assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
    }

    #[test]
    fn raw_opt_source_with_update_and_reinstall_expect_conflict() {
        setup();
        assert_eq!(
            raw_opt_err(&["source", "--update", "--reinstall"]).kind,
            clap::ErrorKind::ArgumentConflict
        );
    }
}
