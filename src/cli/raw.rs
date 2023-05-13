#![deny(missing_docs)]

use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use clap_complete as complete;
use url::Url;

use crate::cli::color_choice::ColorChoice;
use crate::config::{GistRepository, GitHubRepository, GitProtocol, Shell};
use crate::util::build;

const HELP_TEMPLATE: &str = "\
{before-help}{bin} {version}
{author}
{about}

{usage-heading}
{tab}{usage}

{all-args}{after-help}";

#[derive(Debug, PartialEq, Eq, Parser)]
#[clap(
    author,
    version = build::CRATE_RELEASE,
    long_version = build::CRATE_LONG_VERSION,
    about,
    long_about = None,
    help_template = HELP_TEMPLATE,
    disable_help_subcommand(true),
    subcommand_required(true),
)]
pub struct RawOpt {
    /// Suppress any informational output.
    #[clap(long, short)]
    pub quiet: bool,

    /// Suppress any interactive prompts and assume "yes" as the answer.
    #[clap(long)]
    pub non_interactive: bool,

    /// Use verbose output.
    #[clap(long, short)]
    pub verbose: bool,

    /// Output coloring: always, auto, or never.
    #[clap(long, value_name = "WHEN", default_value_t)]
    pub color: ColorChoice,

    /// The configuration directory.
    #[clap(long, value_name = "PATH", env = "SHELDON_CONFIG_DIR")]
    pub config_dir: Option<PathBuf>,

    /// The data directory
    #[clap(long, value_name = "PATH", env = "SHELDON_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// The config file.
    #[clap(long, value_name = "PATH", env = "SHELDON_CONFIG_FILE")]
    pub config_file: Option<PathBuf>,

    /// The profile used for conditional plugins.
    #[clap(long, value_name = "PROFILE", env = "SHELDON_PROFILE")]
    pub profile: Option<String>,

    /// The subcommand to run.
    #[clap(subcommand)]
    pub command: RawCommand,
}

#[derive(Debug, PartialEq, Eq, Parser)]
pub enum RawCommand {
    /// Initialize a new config file.
    Init {
        /// The type of shell, accepted values are: bash, fish, zsh.
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

#[derive(Debug, PartialEq, Eq, Parser)]
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
    #[clap(long = "use", value_name = "MATCH", num_args(1..))]
    pub uses: Option<Vec<String>>,

    /// Templates to apply to this plugin.
    #[clap(long, value_name = "TEMPLATE", num_args(1..))]
    pub apply: Option<Vec<String>>,

    /// Only use this plugin under one of the given profiles
    #[clap(long, value_name = "PROFILES", num_args(1..))]
    pub profiles: Option<Vec<String>>,

    /// Hooks executed during template evaluation.
    #[clap(long, value_name = "SCRIPT", value_parser = key_value_parser, num_args(1..))]
    pub hooks: Option<Vec<(String, String)>>,
}

impl From<Shell> for complete::Shell {
    fn from(s: Shell) -> Self {
        match s {
            Shell::Bash => complete::Shell::Bash,
            Shell::Fish => complete::Shell::Fish,
            Shell::Zsh => complete::Shell::Zsh,
        }
    }
}

fn key_value_parser(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((k, v)) => Ok((k.to_string(), v.to_string())),
        _ => Err(format!(
            "{} isn't a valid key-value pair separated with =",
            s
        )),
    }
}
