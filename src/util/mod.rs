pub mod build;
pub mod git;
mod path_ext;
mod temp;

use std::fs::File;
use std::io;
use std::io::Write;
use std::result;

use anyhow::Error;
use once_cell::sync::Lazy;

pub use crate::util::temp::TempPath;

pub static TEMPLATE_ENGINE: Lazy<upon::Engine> = Lazy::new(upon::Engine::new);

/// Returns the underlying error kind for the given error.
pub fn underlying_io_error_kind(error: &Error) -> Option<io::ErrorKind> {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<io::Error>() {
            return Some(io_error.kind());
        }
    }
    None
}

/// Download a remote file.
pub fn download(url: &str, mut file: File) -> result::Result<(), curl::Error> {
    let mut easy = curl::easy::Easy::new();
    easy.fail_on_error(true)?; // -f
    easy.follow_location(true)?; // -L
    easy.url(url.as_ref())?;
    let mut transfer = easy.transfer();
    transfer.write_function(move |data| {
        match file.write_all(data) {
            Ok(()) => Ok(data.len()),
            Err(_) => Ok(0), // signals to cURL that the writing failed
        }
    })?;
    transfer.perform()?;
    Ok(())
}
