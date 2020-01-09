//! Command line interface.

use std::{path::PathBuf, process};

use structopt::{
    clap::{crate_version, AppSettings},
    StructOpt,
};

use crate::{
    context::Settings,
    error::Error,
    log::{Output, Verbosity},
};

const SETTINGS: &[AppSettings] = &[AppSettings::ColorNever, AppSettings::DeriveDisplayOrder];
const HELP_MESSAGE: &str = "Show this message and exit.";
const VERSION_MESSAGE: &str = "Show the version and exit.";

#[derive(Debug, StructOpt)]
#[structopt(
    author,
    about,
    settings = &SETTINGS,
    setting = AppSettings::DisableHelpSubcommand,
    setting = AppSettings::SubcommandRequired,
    setting = AppSettings::VersionlessSubcommands,
    help_message = HELP_MESSAGE,
    version_message = VERSION_MESSAGE,
)]
struct RawOpt {
    /// Suppress any informational output.
    #[structopt(long, short)]
    quiet: bool,

    /// Use verbose output.
    #[structopt(long, short)]
    verbose: bool,

    /// Do not use ANSI colored output.
    #[structopt(long)]
    no_color: bool,

    /// The home directory.
    #[structopt(long, value_name = "PATH", hidden(true))]
    home: Option<PathBuf>,

    /// The root directory.
    #[structopt(long, value_name = "PATH", env = "SHELDON_ROOT")]
    root: Option<PathBuf>,

    /// The config file.
    #[structopt(long, value_name = "PATH", env = "SHELDON_CONFIG_FILE")]
    config_file: Option<PathBuf>,

    /// The lock file.
    #[structopt(long, value_name = "PATH", env = "SHELDON_LOCK_FILE")]
    lock_file: Option<PathBuf>,

    /// The directory where git sources are cloned to.
    #[structopt(long, value_name = "PATH", env = "SHELDON_CLONE_DIR")]
    clone_dir: Option<PathBuf>,

    /// The directory where remote sources are downloaded to.
    #[structopt(long, value_name = "PATH", env = "SHELDON_DOWNLOAD_DIR")]
    download_dir: Option<PathBuf>,

    /// The subcommand to run.
    #[structopt(subcommand)]
    command: Command,
}

/// The command that is being run.
#[derive(Debug, StructOpt)]
pub enum Command {
    /// Install the plugins sources and generate the lock file.
    #[structopt(settings = &SETTINGS, help_message = HELP_MESSAGE)]
    Lock {
        /// Reinstall all plugin sources.
        #[structopt(long)]
        reinstall: bool,
    },

    /// Generate and print out the script.
    #[structopt(settings = &SETTINGS, help_message = HELP_MESSAGE)]
    Source {
        /// Reinstall all plugin sources.
        #[structopt(long)]
        reinstall: bool,

        /// Regenerate the lock file.
        #[structopt(long)]
        relock: bool,
    },
}

/// Resolved command line options with defaults set.
pub struct Opt {
    /// Global settings for use across the entire program.
    pub settings: Settings,
    /// The output style.
    pub output: Output,
    /// The subcommand.
    pub command: Command,
}

impl Opt {
    /// Gets the struct from the command line arguments. Print the error message
    /// and quit the program in case of failure.
    pub fn from_args() -> Self {
        let RawOpt {
            quiet,
            verbose,
            no_color,
            home,
            root,
            config_file,
            lock_file,
            clone_dir,
            download_dir,
            command,
        } = RawOpt::from_args();

        let verbosity = if quiet {
            Verbosity::Quiet
        } else if verbose {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        };

        let output = Output {
            verbosity,
            no_color,
        };

        let home = match home.or_else(dirs::home_dir).ok_or_else(|| {
            Error::custom(
                "failed to determine the current user's home directory, try using the `--home` \
                 option"
                    .into(),
            )
        }) {
            Ok(home) => home,
            Err(err) => {
                error!(&output, &err);
                process::exit(1);
            }
        };
        let root = root.unwrap_or_else(|| home.join(".zsh"));
        let config_file = config_file.unwrap_or_else(|| root.join("plugins.toml"));
        let lock_file = lock_file.unwrap_or_else(|| config_file.with_extension("lock"));
        let clone_dir = clone_dir.unwrap_or_else(|| root.join("repositories"));
        let download_dir = download_dir.unwrap_or_else(|| root.join("downloads"));

        let settings = Settings {
            version: String::from(crate_version!()),
            home,
            root,
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
}
