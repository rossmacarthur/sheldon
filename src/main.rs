use std::process;

use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand,
};
use log::error;

fn run() -> sheldon::Result<()> {
    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .about(crate_description!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::DisableHelpSubcommand)
        .help_message("Show this message and exit.")
        .version(crate_version!())
        .version_short("v")
        .version_message("Show the version and exit.")
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .short("d")
                .help("Enable debug logging."),
        )
        .arg(
            Arg::with_name("root")
                .long("root")
                .short("r")
                .takes_value(true)
                .help("The root folder."),
        )
        .arg(
            Arg::with_name("config")
                .long("config")
                .short("c")
                .takes_value(true)
                .help("The config file."),
        )
        .subcommand(SubCommand::with_name("add").about("Add a new plugin."))
        .subcommand(SubCommand::with_name("plugins").about("List all the configured plugins."))
        .subcommand(SubCommand::with_name("lock").about("Download all the configured plugins."))
        .subcommand(SubCommand::with_name("source").about("Print out the generated init script."))
        .get_matches();

    sheldon::init_logging(matches.is_present("debug"));

    let ctx = sheldon::Context::defaults(matches.value_of("root"), matches.value_of("config"));

    match matches.subcommand() {
        ("add", _) => error!("this command is not supported yet"),
        ("plugins", _) => error!("this command is not supported yet"),
        ("lock", _) => sheldon::lock(&ctx)?,
        ("source", _) => sheldon::source(&ctx)?,
        _ => unreachable!(),
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        error!("{}", e);
        process::exit(1);
    }
}
