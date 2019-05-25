use std::process;

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
    let settings = [
        AppSettings::ColorNever,
        AppSettings::DeriveDisplayOrder,
        AppSettings::DisableVersion,
    ];

    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .about(crate_description!())
        .setting(AppSettings::ColorNever)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::DisableHelpSubcommand)
        .help_message("Show this message and exit.")
        .version(crate_version!())
        .version_message("Show the version and exit.")
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .multiple(true)
                .help("Set the level of verbosity."),
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
                .about("Lock the configuration.")
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
                        .help("Regenerate the plugins lock file."),
                ),
        )
        .get_matches();

    let app = sheldon::Builder::from_arg_matches(&matches).build();

    match matches.subcommand() {
        ("lock", _) => app.lock()?,
        ("source", _) => print!("{}", app.source()?),
        _ => unreachable!(),
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!(
            "Error: {}",
            format!("{}", e)
                .replace("\n", "\n       ")
                .replace("Template error: ", "")
        );
        process::exit(1);
    }
}
