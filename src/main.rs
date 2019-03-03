use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, SubCommand,
};

fn main() {
    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .about(crate_description!())
        .setting(AppSettings::DisableHelpSubcommand)
        .help_message("Show this message and exit.")
        .version(crate_version!())
        .version_short("v")
        .version_message("Show the version and exit.")
        .subcommand(SubCommand::with_name("add").about("Add a new plugin."))
        .subcommand(SubCommand::with_name("list").about("List all the configured plugins."))
        .subcommand(SubCommand::with_name("install").about("Download all the configured plugins."))
        .subcommand(SubCommand::with_name("source").about("Print out the generated init script."))
        .get_matches();

    match matches.subcommand() {
        ("add", _) => eprintln!("Error: this command is not supported yet"),
        ("list", _) => eprintln!("Error: this command is not supported yet"),
        ("install", _) => eprintln!("Error: this command is not supported yet"),
        ("source", _) => eprintln!("Error: this command is not supported yet"),
        _ => unreachable!(),
    }
}
