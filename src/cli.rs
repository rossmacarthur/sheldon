//! Command line interface.

use std::{path::PathBuf, process};

use anyhow::anyhow;
use structopt::{
    clap::{crate_version, AppSettings, ArgGroup},
    StructOpt,
};
use url::Url;

use crate::{
    config::{GistRepository, GitHubRepository, GitProtocol, GitReference, RawPlugin},
    context::Settings,
    edit::Plugin,
    log::{Output, Verbosity},
};

const SETTINGS: &[AppSettings] = &[
    AppSettings::ColorNever,
    AppSettings::DeriveDisplayOrder,
    AppSettings::DisableHelpSubcommand,
    AppSettings::VersionlessSubcommands,
];
const HELP_MESSAGE: &str = "Show this message and exit";
const VERSION_MESSAGE: &str = "Show the version and exit";

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(
    group = ArgGroup::with_name("plugin").required(true),
    group = ArgGroup::with_name("git-reference")
)]
struct Add {
    /// A unique name for this plugin.
    #[structopt(value_name = "NAME")]
    name: String,

    /// Add a clonable Git repository.
    #[structopt(long, value_name = "URL", group = "plugin")]
    git: Option<Url>,

    /// Add a clonable Gist snippet.
    #[structopt(long, value_name = "ID", group = "plugin")]
    gist: Option<GistRepository>,

    /// Add a clonable GitHub repository.
    #[structopt(long, value_name = "REPO", group = "plugin")]
    github: Option<GitHubRepository>,

    /// Add a downloadable file.
    #[structopt(long, value_name = "URL", group = "plugin")]
    remote: Option<Url>,

    /// Add a local directory.
    #[structopt(long, value_name = "DIR", group = "plugin")]
    local: Option<PathBuf>,

    /// The Git protocol for a Gist or GitHub plugin.
    #[structopt(long, value_name = "PROTO", conflicts_with_all = &["git", "remote", "local"])]
    proto: Option<GitProtocol>,

    /// Checkout the tip of a branch.
    #[structopt(
        long,
        value_name = "BRANCH",
        group = "git-reference",
        // for some weird reason this makes all 'git-reference' options correctly conflict
        // but putting it on the 'git-reference' ArgGroup doesn't work
        conflicts_with_all = &["remote", "local"],
    )]
    branch: Option<String>,

    /// Checkout a specific commit.
    #[structopt(long, value_name = "SHA", group = "git-reference")]
    rev: Option<String>,

    /// Checkout a specific tag.
    #[structopt(long, value_name = "TAG", group = "git-reference")]
    tag: Option<String>,

    /// Which sub directory to use in this plugin.
    #[structopt(long, value_name = "PATH")]
    dir: Option<String>,

    /// Which files to use in this plugin.
    #[structopt(long = "use", value_name = "MATCH")]
    uses: Option<Vec<String>>,

    /// Templates to apply to this plugin.
    #[structopt(long, value_name = "TEMPLATE")]
    apply: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, StructOpt)]
enum RawCommand {
    /// Add a new plugin to the config file.
    #[structopt(help_message = HELP_MESSAGE)]
    Add(Box<Add>),

    /// Open up the config file in the default editor.
    Edit,

    /// Remove a plugin from the config file.
    #[structopt(help_message = HELP_MESSAGE)]
    Remove {
        /// A unique name for this plugin.
        #[structopt(value_name = "NAME")]
        name: String,
    },

    /// Install the plugins sources and generate the lock file.
    #[structopt(help_message = HELP_MESSAGE)]
    Lock {
        /// Reinstall all plugin sources.
        #[structopt(long)]
        reinstall: bool,
    },

    /// Generate and print out the script.
    #[structopt(help_message = HELP_MESSAGE)]
    Source {
        /// Regenerate the lock file.
        #[structopt(long)]
        relock: bool,
        /// Reinstall all plugin sources (implies --relock).
        #[structopt(long)]
        reinstall: bool,
    },
}

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(
    author,
    about,
    setting = AppSettings::SubcommandRequired,
    global_settings = &SETTINGS,
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
    command: RawCommand,
}

/// The resolved command.
#[derive(Debug)]
pub enum Command {
    /// Add a new plugin to the config file.
    Add { name: String, plugin: Box<Plugin> },
    /// Open up the config file in the default editor.
    Edit,
    /// Remove a plugin from the config file.
    Remove { name: String },
    /// Install the plugins sources and generate the lock file.
    Lock { reinstall: bool },
    /// Generate and print out the script.
    Source { reinstall: bool, relock: bool },
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
            no_color,
            home,
            root,
            config_file,
            lock_file,
            clone_dir,
            download_dir,
            command,
        } = raw_opt;

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
        let root = root.unwrap_or_else(|| home.join(".sheldon"));
        let config_file = config_file.unwrap_or_else(|| root.join("plugins.toml"));
        let lock_file = lock_file.unwrap_or_else(|| config_file.with_extension("lock"));
        let clone_dir = clone_dir.unwrap_or_else(|| root.join("repos"));
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

        let command = match command {
            RawCommand::Add(add) => {
                let (name, plugin) = Plugin::from_add(*add);
                Command::Add {
                    name,
                    plugin: Box::new(plugin),
                }
            }
            RawCommand::Edit => Command::Edit,
            RawCommand::Remove { name } => Command::Remove { name },
            RawCommand::Lock { reinstall } => Command::Lock { reinstall },
            RawCommand::Source { relock, reinstall } => Command::Source {
                relock: relock || reinstall,
                reinstall,
            },
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
        Self::from_raw_opt(RawOpt::from_args())
    }
}

/////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::{env, iter};
    use structopt::clap::{crate_authors, crate_description, crate_name};

    fn setup() {
        for (k, _) in env::vars() {
            if k.starts_with(&format!("{}_", crate_name!().to_uppercase())) {
                env::remove_var(k);
            }
        }
    }

    fn raw_opt(args: &[&str]) -> RawOpt {
        RawOpt::from_iter_safe(iter::once(crate_name!()).chain(args.into_iter().map(|s| *s)))
            .unwrap()
    }

    fn raw_opt_err(args: &[&str]) -> structopt::clap::Error {
        RawOpt::from_iter_safe(iter::once(crate_name!()).chain(args.into_iter().map(|s| *s)))
            .unwrap_err()
    }

    #[test]
    fn raw_opt_version() {
        setup();
        for opt in &["-V", "--version"] {
            let err = raw_opt_err(&[opt]);
            assert_eq!(err.message, ""); // not sure why this doesn't contain the outputted data :/
            assert_eq!(err.kind, structopt::clap::ErrorKind::VersionDisplayed);
            assert_eq!(err.info, None);
        }
    }

    #[test]
    fn raw_opt_help() {
        setup();
        for opt in &["-h", "--help"] {
            let err = raw_opt_err(&[opt]);
            assert_eq!(
                err.message,
                format!(
                    "\
{name} {version}
{authors}
{description}

USAGE:
    {name} [FLAGS] [OPTIONS] <SUBCOMMAND>

FLAGS:
    -q, --quiet       Suppress any informational output
    -v, --verbose     Use verbose output
        --no-color    Do not use ANSI colored output
    -h, --help        Show this message and exit
    -V, --version     Show the version and exit

OPTIONS:
        --root <PATH>            The root directory [env: SHELDON_ROOT=]
        --config-file <PATH>     The config file [env: SHELDON_CONFIG_FILE=]
        --lock-file <PATH>       The lock file [env: SHELDON_LOCK_FILE=]
        --clone-dir <PATH>       The directory where git sources are cloned to [env: \
                     SHELDON_CLONE_DIR=]
        --download-dir <PATH>    The directory where remote sources are downloaded to [env: \
                     SHELDON_DOWNLOAD_DIR=]

SUBCOMMANDS:
    add       Add a new plugin to the config file
    edit      Open up the config file in the default editor
    remove    Remove a plugin from the config file
    lock      Install the plugins sources and generate the lock file
    source    Generate and print out the script",
                    name = crate_name!(),
                    version = crate_version!(),
                    authors = crate_authors!(),
                    description = crate_description!(),
                )
            );
            assert_eq!(err.kind, structopt::clap::ErrorKind::HelpDisplayed);
            assert_eq!(err.info, None);
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
                no_color: false,
                home: None,
                root: None,
                config_file: None,
                lock_file: None,
                clone_dir: None,
                download_dir: None,
                command: RawCommand::Lock { reinstall: false },
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
                "--no-color",
                "--home",
                "/",
                "--root",
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
                no_color: true,
                home: Some("/".into()),
                root: Some("/test".into()),
                config_file: Some("/plugins.toml".into()),
                lock_file: Some("/test/plugins.lock".into()),
                clone_dir: Some("/repos".into()),
                download_dir: Some("/downloads".into()),
                command: RawCommand::Lock { reinstall: false },
            }
        );
    }

    #[test]
    fn raw_opt_subcommand_required() {
        setup();
        let err = raw_opt_err(&[]);
        assert_eq!(
            err.message,
            format!(
                "\
error: '{name}' requires a subcommand, but one was not provided

USAGE:
    {name} [FLAGS] [OPTIONS] <SUBCOMMAND>

For more information try --help",
                name = crate_name!()
            )
        );
        assert_eq!(err.kind, structopt::clap::ErrorKind::MissingSubcommand);
        assert_eq!(err.info, None);
    }

    #[test]
    fn raw_opt_add_help() {
        setup();
        let err = raw_opt_err(&["add", "--help"]);
        assert_eq!(
            err.message,
            format!(
                "\
{name}-add {version}
Add a new plugin to the config file

USAGE:
    {name} add [OPTIONS] <NAME> <--git <URL>|--gist <ID>|--github <REPO>|--remote <URL>|--local \
                 <DIR>>

FLAGS:
    -h, --help    Show this message and exit

OPTIONS:
        --git <URL>              Add a clonable Git repository
        --gist <ID>              Add a clonable Gist snippet
        --github <REPO>          Add a clonable GitHub repository
        --remote <URL>           Add a downloadable file
        --local <DIR>            Add a local directory
        --proto <PROTO>          The Git protocol for a Gist or GitHub plugin
        --branch <BRANCH>        Checkout the tip of a branch
        --rev <SHA>              Checkout a specific commit
        --tag <TAG>              Checkout a specific tag
        --dir <PATH>             Which sub directory to use in this plugin
        --use <MATCH>...         Which files to use in this plugin
        --apply <TEMPLATE>...    Templates to apply to this plugin

ARGS:
    <NAME>    A unique name for this plugin",
                name = crate_name!(),
                version = crate_version!()
            )
        );
        assert_eq!(err.kind, structopt::clap::ErrorKind::HelpDisplayed);
        assert_eq!(err.info, None);
    }

    #[test]
    fn raw_opt_add_no_source() {
        setup();
        assert_eq!(
            raw_opt_err(&["add", "test",]).kind,
            structopt::clap::ErrorKind::MissingRequiredArgument
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
            structopt::clap::ErrorKind::ArgumentConflict
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
            structopt::clap::ErrorKind::ArgumentConflict
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
            structopt::clap::ErrorKind::ArgumentConflict
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
            structopt::clap::ErrorKind::ArgumentConflict
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
            structopt::clap::ErrorKind::ArgumentConflict
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
            structopt::clap::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn raw_opt_lock_help() {
        setup();
        let err = raw_opt_err(&["lock", "--help"]);
        assert_eq!(
            err.message,
            format!(
                "\
{name}-lock {version}
Install the plugins sources and generate the lock file

USAGE:
    {name} lock [FLAGS]

FLAGS:
        --reinstall    Reinstall all plugin sources
    -h, --help         Show this message and exit",
                name = crate_name!(),
                version = crate_version!()
            )
        );
        assert_eq!(err.kind, structopt::clap::ErrorKind::HelpDisplayed);
        assert_eq!(err.info, None);
    }

    #[test]
    fn raw_opt_source_help() {
        setup();
        let err = raw_opt_err(&["source", "--help"]);
        assert_eq!(
            err.message,
            format!(
                "\
{name}-source {version}
Generate and print out the script

USAGE:
    {name} source [FLAGS]

FLAGS:
        --relock       Regenerate the lock file
        --reinstall    Reinstall all plugin sources (implies --relock)
    -h, --help         Show this message and exit",
                name = crate_name!(),
                version = crate_version!()
            )
        );
        assert_eq!(err.kind, structopt::clap::ErrorKind::HelpDisplayed);
        assert_eq!(err.info, None);
    }
}
