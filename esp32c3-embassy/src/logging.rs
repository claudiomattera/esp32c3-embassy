// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Functions for setting up the logging system

use core::str::FromStr;

use log::max_level;
use log::set_logger_racy;
use log::set_max_level_racy;
use log::trace;
use log::Level;
use log::LevelFilter;
use log::Log;
use log::Metadata;
use log::Record;

use esp_println::println;

/// Setup logging
///
/// To change the log level change the `env` section in `.cargo/config.toml`
/// or remove it and set the environment variable `ESP_LOGLEVEL` manually
/// before running `cargo run`.
///
/// This requires a clean rebuild because of
/// <https://github.com/rust-lang/cargo/issues/10358>
pub fn setup() {
    /// Log level
    const LEVEL: Option<&'static str> = option_env!("ESP_LOGLEVEL");

    // SAFETY:
    //
    let result = unsafe { set_logger_racy(&EspPrintlnLogger) };

    // SAFETY:
    //
    unsafe { result.unwrap_unchecked() };

    if let Some(lvl) = LEVEL {
        let level = LevelFilter::from_str(lvl).unwrap_or(LevelFilter::Off);

        // SAFETY:
        //
        unsafe { set_max_level_racy(level) };
    }

    trace!("Logger is ready");
}

/// Logger that prints messages to console
struct EspPrintlnLogger;

impl Log for EspPrintlnLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if metadata.target().starts_with("esp_wifi") {
            metadata.level() <= Level::Info
        } else {
            metadata.level() <= max_level()
        }
    }

    fn log(&self, record: &Record) {
        /// Modifier for restoring normal text style
        const RESET: &str = "\u{001B}[0m";
        /// Modifier for setting gray text
        const GRAY: &str = "\u{001B}[2m";
        /// Modifier for setting red text
        const RED: &str = "\u{001B}[31m";
        /// Modifier for setting green text
        const GREEN: &str = "\u{001B}[32m";
        /// Modifier for setting yellow text
        const YELLOW: &str = "\u{001B}[33m";
        /// Modifier for setting blue text
        const BLUE: &str = "\u{001B}[34m";
        /// Modifier for setting cyan text
        const CYAN: &str = "\u{001B}[35m";

        let color = match record.level() {
            Level::Error => RED,
            Level::Warn => YELLOW,
            Level::Info => GREEN,
            Level::Debug => BLUE,
            Level::Trace => CYAN,
        };

        if self.enabled(record.metadata()) {
            println!(
                "{}{:>5} {}{}{}{}]{} {}",
                color,
                record.level(),
                RESET,
                GRAY,
                record.target(),
                GRAY,
                RESET,
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
