use std::{panic, process};

use ansi_term::Color;
use clap::{
    crate_authors,
    crate_description,
    crate_name,
    crate_version,
    App,
    AppSettings,
    Arg,
    SubCommand,
};

fn run() -> sheldon::Result<()> {
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
                .help("Suppresses any output."),
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

    let (subcommand, submatches) = matches.subcommand();
    let app = sheldon::Builder::from_arg_matches(&matches, submatches.unwrap()).build();

    match subcommand {
        "lock" => app.lock()?,
        "source" => print!("{}", app.source()?),
        _ => unimplemented!(),
    }

    Ok(())
}

fn main() {
    if let Err(_) = panic::catch_unwind(|| {
        if let Err(e) = run() {
            eprintln!(
                "\n{} {}",
                Color::Red.bold().paint("error:"),
                format!("{}", e)
                    .replace("\n", "\n  due to: ")
                    // .replace("\n", "\n       ")
                    .replace("Template error: ", "")
            );
            process::exit(1);
        }
    }) {
        eprintln!("\nThis is probably a bug, please file an issue at \
                     https://github.com/rossmacarthur/sheldon/issues.");
        process::exit(2);
    }
}
