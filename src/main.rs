use std::process;

use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand,
};
use log::{debug, error};

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
        .subcommand(SubCommand::with_name("add").about("Add a new plugin."))
        .subcommand(SubCommand::with_name("list").about("List all the configured plugins."))
        .subcommand(SubCommand::with_name("install").about("Download all the configured plugins."))
        .subcommand(SubCommand::with_name("source").about("Print out the generated init script."))
        .get_matches();

    sheldon::init_logger(matches.is_present("debug"));

    debug!("debug logging is enabled!");

    match matches.subcommand() {
        ("add", _) => error!("this command is not supported yet"),
        ("list", _) => error!("this command is not supported yet"),
        ("install", _) => error!("this command is not supported yet"),
        ("source", _) => error!("this command is not supported yet"),
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
