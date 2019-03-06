use ansi_term::Color;
use log::{Level, Log, Metadata, Record};

/// Our logging struct.
struct Logger;

/// Create a global variable for the logger.
static LOGGER: Logger = Logger;

impl Log for Logger {
    /// Our logger is always enabled.
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    /// Actually log the message.
    fn log(&self, record: &Record) {
        let level = record.metadata().level();

        let color = match level {
            Level::Error => Color::Red,
            Level::Warn => Color::Yellow,
            Level::Info => Color::Green,
            _ => Color::White,
        };
        if self.enabled(record.metadata()) {
            eprintln!(
                "{} {}",
                color
                    .bold()
                    .paint(format!("{: >5}:", level.to_string().to_lowercase())),
                format!("{}", record.args()).replace("\n", "\n         ") // indent multiline
            );
        }
    }

    fn flush(&self) {}
}

/// Initialize the global logger.
///
/// # Panics
///
/// Panics if this function is called a second time, or a global logger has
/// already been set.
///
/// # Examples
///
/// ```
/// sheldon::init_logging(true); // enable debug logging
/// ```
pub fn init_logging(debug: bool) {
    log::set_logger(&LOGGER).expect("failed to set logger");
    if debug {
        log::set_max_level(log::LevelFilter::Debug)
    } else {
        log::set_max_level(log::LevelFilter::Info)
    }
}
