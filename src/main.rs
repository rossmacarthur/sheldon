use std::{panic, process};

use ansi_term::Color;
use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg, ArgMatches,
    SubCommand,
};

fn run_subcommand(matches: ArgMatches) {
    if let Err(e) = || -> sheldon::Result<()> {
        let (subcommand, submatches) = matches.subcommand();
        let app = sheldon::Builder::from_arg_matches(&matches, submatches.unwrap()).build();

        match subcommand {
            "lock" => app.lock()?,
            "source" => print!("{}", app.source()?),
            _ => unreachable!(),
        }

        Ok(())
    }() {
        let as_string = format!("{}", e)
            .replace("\n", "\n  due to: ")
            .replace("Template error: ", "");

        if matches.is_present("no-color") {
            eprintln!("\n[ERROR] {}", as_string);
        } else {
            eprintln!("\n{} {}", Color::Red.bold().paint("error:"), as_string);
        }

        process::exit(1);
    }
}

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
            Arg::with_name("quiet")
                .long("quiet")
                .short("q")
                .help("Suppress any informational output."),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Use verbose output."),
        )
        .arg(
            Arg::with_name("no-color")
                .long("no-color")
                .help("Do not use ANSI colored output."),
        )
        .arg(
            Arg::with_name("home")
                .long("home")
                .takes_value(true)
                .hidden(true)
                .help("The current user's home directory."),
        )
        .arg(
            Arg::with_name("root")
                .long("root")
                .takes_value(true)
                .help("Override the root directory."),
        )
        .arg(
            Arg::with_name("config-file")
                .long("config-file")
                .takes_value(true)
                .help("Override the config file."),
        )
        .arg(
            Arg::with_name("lock-file")
                .long("lock-file")
                .takes_value(true)
                .help("Override the lock file."),
        )
        .subcommand(
            SubCommand::with_name("lock")
                .about("Install the plugins sources and generate the lock file.")
                .settings(&settings)
                .arg(
                    Arg::with_name("reinstall")
                        .long("reinstall")
                        .help("Reinstall all plugin sources."),
                ),
        )
        .subcommand(
            SubCommand::with_name("source")
                .about("Generate and print out the script.")
                .settings(&settings)
                .arg(
                    Arg::with_name("reinstall")
                        .long("reinstall")
                        .help("Reinstall all plugin sources."),
                )
                .arg(
                    Arg::with_name("relock")
                        .long("relock")
                        .help("Regenerate the lock file."),
                ),
        )
        .get_matches();

    if panic::catch_unwind(|| run_subcommand(matches)).is_err() {
        eprintln!(
            "\nThis is probably a bug, please file an issue at \
             https://github.com/rossmacarthur/sheldon/issues."
        );
        process::exit(2);
    }
}
