mod _macros;
mod app;
mod build;
mod cli;
mod config;
mod context;
mod edit;
mod editor;
mod lock;
mod log;
mod util;

use std::panic;
use std::process;

fn run() {
    if crate::app::run().is_err() {
        process::exit(2);
    }
}

fn main() {
    if panic::catch_unwind(run).is_err() {
        eprintln!(
            "\nThis is probably a bug, please file an issue at \
             https://github.com/rossmacarthur/sheldon/issues."
        );
        process::exit(127);
    }
}
