//! Error types for the Chaff CLI

// Not unit testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt};

use chaff_capture::errors::{CaptureError, TraceError};

/// CLI errors
#[derive(Debug)]
pub enum CliError {
    /// Device specified but not found
    DeviceNotFound(String),

    /// Errors from the chaff-capture crate
    Capture(CaptureError),
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";

impl Error for CliError {
    fn cause(&self) -> Option<&dyn Error> {
        #[expect(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::Capture(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{BOLD}{RED}error:{RESET} ")?;
        match self {
            Self::DeviceNotFound(device) => write!(f, "device specified but not found: {device}"),
            Self::Capture(e) => write!(f, "capture error: {e}"),
        }
    }
}

impl From<CaptureError> for CliError {
    fn from(e: CaptureError) -> Self {
        Self::Capture(e)
    }
}

impl From<TraceError> for CliError {
    fn from(e: TraceError) -> Self {
        Self::Capture(CaptureError::Trace(e))
    }
}
