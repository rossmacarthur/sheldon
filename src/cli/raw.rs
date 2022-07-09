use std::path::PathBuf;

use clap::{AppSettings, ArgGroup, Parser};
use clap_complete as complete;
use url::Url;

use crate::cli::color_choice::ColorChoice;
use crate::config::{GistRepository, GitHubRepository, GitProtocol, Shell};
use crate::util::build;

#[derive(Debug, PartialEq, Parser)]
#[clap(
    author,
    about,
    version = build::CRATE_RELEASE,
    long_version = build::CRATE_LONG_VERSION,
    term_width = 120,
    global_setting = AppSettings::DeriveDisplayOrder,
    global_setting = AppSettings::DisableHelpSubcommand,
    global_setting = AppSettings::DisableColoredHelp,
    setting = AppSettings::SubcommandRequired,
)]
pub struct RawOpt {
    /// Suppress any informational output.
    #[clap(long, short)]
    pub quiet: bool,

    /// Use verbose output.
    #[clap(long, short)]
    pub verbose: bool,

    /// Output coloring: always, auto, or never.
    #[clap(long, value_name = "WHEN", default_value_t)]
    pub color: ColorChoice,

    /// The home directory.
    #[clap(long, value_name = "PATH", hide(true))]
    pub home: Option<PathBuf>,

    /// The configuration directory.
    #[clap(long, value_name = "PATH", env = "SHELDON_CONFIG_DIR")]
    pub config_dir: Option<PathBuf>,

    /// The data directory
    #[clap(long, value_name = "PATH", env = "SHELDON_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// The config file.
    #[clap(long, value_name = "PATH", env = "SHELDON_CONFIG_FILE")]
    pub config_file: Option<PathBuf>,

    /// The lock file.
    #[clap(long, value_name = "PATH", env = "SHELDON_LOCK_FILE")]
    pub lock_file: Option<PathBuf>,

    /// The directory where git sources are cloned to.
    #[clap(long, value_name = "PATH", env = "SHELDON_CLONE_DIR")]
    pub clone_dir: Option<PathBuf>,

    /// The directory where remote sources are downloaded to.
    #[clap(long, value_name = "PATH", env = "SHELDON_DOWNLOAD_DIR")]
    pub download_dir: Option<PathBuf>,

    /// The profile used for conditional plugins.
    #[clap(long, value_name = "PROFILE", env = "SHELDON_PROFILE")]
    pub profile: Option<String>,

    /// The subcommand to run.
    #[clap(subcommand)]
    pub command: RawCommand,
}

#[derive(Debug, PartialEq, Parser)]
pub enum RawCommand {
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

    /// Generate completions for the given shell.
    Completions {
        /// The type of shell, accepted values are: bash, zsh.
        #[clap(long, value_name = "SHELL")]
        shell: Shell,
    },

    /// Prints detailed version information.
    Version,
}

#[derive(Debug, PartialEq, Parser)]
#[clap(
    group = ArgGroup::new("plugin").required(true),
    group = ArgGroup::new("git-reference").conflicts_with_all(&["remote", "local"]),
)]
pub struct Add {
    /// A unique name for this plugin.
    #[clap(value_name = "NAME")]
    pub name: String,

    /// Add a clonable Git repository.
    #[clap(long, value_name = "URL", group = "plugin")]
    pub git: Option<Url>,

    /// Add a clonable Gist snippet.
    #[clap(long, value_name = "ID", group = "plugin")]
    pub gist: Option<GistRepository>,

    /// Add a clonable GitHub repository.
    #[clap(long, value_name = "REPO", group = "plugin")]
    pub github: Option<GitHubRepository>,

    /// Add a downloadable file.
    #[clap(long, value_name = "URL", group = "plugin")]
    pub remote: Option<Url>,

    /// Add a local directory.
    #[clap(long, value_name = "DIR", group = "plugin")]
    pub local: Option<PathBuf>,

    /// The Git protocol for a Gist or GitHub plugin.
    #[clap(long, value_name = "PROTO", conflicts_with_all = &["git", "remote", "local"])]
    pub proto: Option<GitProtocol>,

    /// Checkout the tip of a branch.
    #[clap(long, value_name = "BRANCH", group = "git-reference")]
    pub branch: Option<String>,

    /// Checkout a specific commit.
    #[clap(long, value_name = "SHA", group = "git-reference")]
    pub rev: Option<String>,

    /// Checkout a specific tag.
    #[clap(long, value_name = "TAG", group = "git-reference")]
    pub tag: Option<String>,

    /// Which sub directory to use in this plugin.
    #[clap(long, value_name = "PATH")]
    pub dir: Option<String>,

    /// Which files to use in this plugin.
    #[clap(long = "use", value_name = "MATCH", multiple_values(true))]
    pub uses: Option<Vec<String>>,

    /// Templates to apply to this plugin.
    #[clap(long, value_name = "TEMPLATE", multiple_values(true))]
    pub apply: Option<Vec<String>>,

    /// Only use this plugin under one of the given profiles
    #[clap(long, value_name = "PROFILES", multiple_values(true))]
    pub profiles: Option<Vec<String>>,
}

impl From<Shell> for complete::Shell {
    fn from(s: Shell) -> Self {
        match s {
            Shell::Bash => complete::Shell::Bash,
            Shell::Zsh => complete::Shell::Zsh,
        }
    }
}
