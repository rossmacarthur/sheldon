//! Command line interface.

mod color_choice;
mod raw;

#[cfg(test)]
mod tests;

use std::io;

use std::path::{Path, PathBuf};
use std::process;

use anyhow::anyhow;
use clap::{IntoApp, Parser};
use clap_complete as complete;

use crate::cli::raw::{Add, RawCommand, RawOpt};
use crate::config::{EditPlugin, GitReference, RawPlugin, Shell};
use crate::context::{log_error, Color, Context, Output, Verbosity};
use crate::lock::LockMode;
use crate::util::build;

/// Parse the command line arguments.
///
/// In the event of failure it will print the error message and quit the program
/// without returning.
pub fn from_args() -> Opt {
    Opt::from_raw_opt(RawOpt::parse())
}

/// Resolved command line options with defaults set.
#[derive(Debug)]
pub struct Opt {
    /// Global context for use across the entire program.
    pub ctx: Context,
    /// The subcommand.
    pub command: Command,
}

/// The resolved command.
#[derive(Debug)]
pub enum Command {
    /// Initialize a new config file.
    Init { shell: Option<Shell> },
    /// Add a new plugin to the config file.
    Add {
        name: String,
        plugin: Box<EditPlugin>,
    },
    /// Open up the config file in the default editor.
    Edit,
    /// Remove a plugin from the config file.
    Remove { name: String },
    /// Install the plugins sources and generate the lock file.
    Lock,
    /// Generate and print out the script.
    Source,
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
            profile,
            command,
        } = raw_opt;

        let mut lock_mode = None;

        let command = match command {
            RawCommand::Init { shell } => Command::Init { shell },
            RawCommand::Add(add) => {
                let (name, plugin) = EditPlugin::from_add(*add);
                Command::Add {
                    name,
                    plugin: Box::new(plugin),
                }
            }
            RawCommand::Edit => Command::Edit,
            RawCommand::Remove { name } => Command::Remove { name },
            RawCommand::Lock { update, reinstall } => {
                lock_mode = LockMode::from_lock_flags(update, reinstall);
                Command::Lock
            }
            RawCommand::Source {
                relock,
                update,
                reinstall,
            } => {
                lock_mode = LockMode::from_source_flags(relock, update, reinstall);
                Command::Source
            }
            RawCommand::Completions { shell } => {
                let mut app = RawOpt::into_app();
                let shell = complete::Shell::from(shell);
                clap_complete::generate(shell, &mut app, build::CRATE_NAME, &mut io::stdout());
                process::exit(0);
            }
            RawCommand::Version => {
                println!("{} {}", build::CRATE_NAME, build::CRATE_VERBOSE_VERSION);
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

        let log_no_home = || {
            let err = anyhow!(
                "failed to determine the current user's home directory, try setting the $HOME \
                 environment variable"
            );
            log_error(output.no_color, Color::Red, "error", &err);
        };

        let base_dirs = match directories::BaseDirs::new() {
            Some(base_dirs) => base_dirs,
            None => {
                log_no_home();
                process::exit(1);
            }
        };

        let project_dirs = match directories::ProjectDirs::from("rs.cli", "", "sheldon") {
            Some(project_dirs) => project_dirs,
            None => {
                log_no_home();
                process::exit(1);
            }
        };

        let home = home.unwrap_or_else(|| base_dirs.home_dir().to_path_buf());

        let (config_dir, old_config_dir) = (
            config_dir
                .clone()
                .unwrap_or_else(|| project_dirs.config_dir().to_path_buf()),
            config_dir.unwrap_or_else(|| home.join(".sheldon")),
        );
        let (data_dir, old_data_dir) = (
            data_dir
                .clone()
                .unwrap_or_else(|| project_dirs.data_dir().to_path_buf()),
            data_dir.unwrap_or_else(|| home.join(".sheldon")),
        );

        let config_file = config_file
            .unwrap_or_else(|| get_and_migrate_dir(&old_config_dir, &config_dir, "plugins.toml"));
        let lock_file = lock_file
            .unwrap_or_else(|| get_and_migrate_dir(&old_data_dir, &data_dir, "plugins.lock"));
        let clone_dir =
            clone_dir.unwrap_or_else(|| get_and_migrate_dir(&old_data_dir, &data_dir, "repos"));
        let download_dir = download_dir
            .unwrap_or_else(|| get_and_migrate_dir(&old_data_dir, &data_dir, "downloads"));

        let ctx = Context {
            version: build::CRATE_RELEASE.to_string(),
            home,
            config_dir,
            data_dir,
            config_file,
            lock_file,
            clone_dir,
            download_dir,
            profile,
            output,
            lock_mode,
        };

        Self { ctx, command }
    }
}

fn get_and_migrate_dir<P: AsRef<Path> + PartialEq>(
    old_parent: P,
    new_parent: P,
    child: &str,
) -> PathBuf {
    if old_parent == new_parent {
        return new_parent.as_ref().join(child);
    }

    let old = old_parent.as_ref().join(child);
    let new = new_parent.as_ref().join(child);

    match (old.exists(), new.exists()) {
        (true, false) => {
            // TODO: log warning if this fails
            std::fs::rename(&old, &new);
            new
        }
        // TODO: log warning here
        (true, true) => new,
        (false, _) => new,
    }
}

impl EditPlugin {
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
            profiles,
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
                profiles,
                rest: None,
            }),
        )
    }
}

impl LockMode {
    fn from_lock_flags(update: bool, reinstall: bool) -> Option<Self> {
        match (update, reinstall) {
            (false, false) => Some(Self::Normal),
            (true, false) => Some(Self::Update),
            (false, true) => Some(Self::Reinstall),
            (true, true) => unreachable!(),
        }
    }

    fn from_source_flags(relock: bool, update: bool, reinstall: bool) -> Option<Self> {
        match (relock, update, reinstall) {
            (false, false, false) => None,
            (true, false, false) => Some(Self::Normal),
            (_, true, false) => Some(Self::Update),
            (_, false, true) => Some(Self::Reinstall),
            (_, true, true) => unreachable!(),
        }
    }
}
