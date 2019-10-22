use std::{panic, process};

use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand,
};

fn main() {
    let settings = [AppSettings::ColorNever, AppSettings::DeriveDisplayOrder];

    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .about(crate_description!())
        .settings(&settings)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::SubcommandRequired)
        .setting(AppSettings::VersionlessSubcommands)
        .help_message("Show this message and exit.")
        .version(crate_version!())
        .version_message("Show the version and exit.")
        .arg(
            Arg::with_name(sheldon::cli::QUIET)
                .long("quiet")
                .short("q")
                .help("Suppress any informational output."),
        )
        .arg(
            Arg::with_name(sheldon::cli::VERBOSE)
                .long("verbose")
                .short("v")
                .help("Use verbose output."),
        )
        .arg(
            Arg::with_name(sheldon::cli::NO_COLOR)
                .long("no-color")
                .help("Do not use ANSI colored output."),
        )
        .arg(
            Arg::with_name(sheldon::cli::HOME)
                .long("home")
                .takes_value(true)
                .value_name("PATH")
                .help("Override the home directory."),
        )
        .arg(
            Arg::with_name(sheldon::cli::ROOT)
                .long("root")
                .takes_value(true)
                .value_name("PATH")
                .help("Override the root directory."),
        )
        .arg(
            Arg::with_name(sheldon::cli::CONFIG_FILE)
                .long("config-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Override the config file."),
        )
        .arg(
            Arg::with_name(sheldon::cli::LOCK_FILE)
                .long("lock-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Override the lock file."),
        )
        .arg(
            Arg::with_name(sheldon::cli::CLONE_DIR)
                .long("clone-dir")
                .takes_value(true)
                .value_name("PATH")
                .help("Override the directory where git sources are cloned to."),
        )
        .arg(
            Arg::with_name(sheldon::cli::DOWNLOAD_DIR)
                .long("download-dir")
                .takes_value(true)
                .value_name("PATH")
                .help("Override the directory where remote sources are downloaded to."),
        )
        .subcommand(
            SubCommand::with_name(sheldon::cli::LOCK)
                .about("Install the plugins sources and generate the lock file.")
                .settings(&settings)
                .arg(
                    Arg::with_name(sheldon::cli::REINSTALL)
                        .long("reinstall")
                        .help("Reinstall all plugin sources."),
                ),
        )
        .subcommand(
            SubCommand::with_name(sheldon::cli::SOURCE)
                .about("Generate and print out the script.")
                .settings(&settings)
                .arg(
                    Arg::with_name(sheldon::cli::REINSTALL)
                        .long("reinstall")
                        .help("Reinstall all plugin sources."),
                )
                .arg(
                    Arg::with_name(sheldon::cli::RELOCK)
                        .long("relock")
                        .help("Regenerate the lock file."),
                ),
        )
        .get_matches();

    let run_command = || {
        let (subcommand, submatches) = matches.subcommand();
        if sheldon::Builder::from_clap(&matches, subcommand, submatches.unwrap())
            .build()
            .run()
            .is_err()
        {
            process::exit(1);
        }
    };

    if panic::catch_unwind(run_command).is_err() {
        eprintln!(
            "\nThis is probably a bug, please file an issue at \
             https://github.com/rossmacarthur/sheldon/issues."
        );
        process::exit(2);
    }
}
