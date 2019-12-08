use std::{panic, process};

fn run() {
    if sheldon::Sheldon::run().is_err() {
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
