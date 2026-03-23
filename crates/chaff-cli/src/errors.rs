//! Error types for the Chaff CLI

// Not unit testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt};

use chaff_capture::errors::ChaffError;

/// CLI errors
#[derive(Debug)]
pub enum CliError {
    /// Device specified but not found
    DeviceNotFound(String),

    /// Errors from the Chaff library
    Library(ChaffError),
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";

impl Error for CliError {
    fn cause(&self) -> Option<&dyn Error> {
        #[expect(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::Library(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{BOLD}{RED}error:{RESET} ")?;
        match self {
            Self::DeviceNotFound(device) => write!(f, "device specified but not found: {device}"),
            Self::Library(e) => write!(f, "library error: {e}"),
        }
    }
}

impl From<ChaffError> for CliError {
    fn from(e: ChaffError) -> Self {
        Self::Library(e)
    }
}
