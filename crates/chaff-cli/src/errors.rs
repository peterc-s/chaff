//! Error types for the Chaff CLI

// Not unit testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt};

use chaff_capture::errors::{CaptureError, TraceError};
use chaff_datasets::errors::ParseError;
use mac_address::{MacAddressError, MacParseError};

/// CLI errors
#[derive(Debug)]
pub enum CliError {
    /// Device specified but not found.
    DeviceNotFound(String),

    /// Errors from the chaff-capture crate.
    Capture(CaptureError),

    /// Errors from the [`mac_address`] crate's [`MacAddressError`].
    MacAddress(MacAddressError),

    /// Errors from the [`mac_address`] crate's [`MacParseError`].
    MacParse(MacParseError),

    /// Errors from the [`pcap`] crate.
    Pcap(pcap::Error),

    /// Dataset type given is not known/implemented.
    UnknownDatasetType(String),

    /// Error from a [`chaff_datasets`] parser.
    Dataset(ParseError),
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";

impl Error for CliError {
    fn cause(&self) -> Option<&dyn Error> {
        match self {
            Self::Capture(e) => Some(e),
            Self::MacAddress(e) => Some(e),
            Self::MacParse(e) => Some(e),
            Self::Pcap(e) => Some(e),
            Self::Dataset(e) => Some(e),
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
            Self::MacAddress(e) => write!(f, "MAC address error: {e}"),
            Self::MacParse(e) => write!(f, "MAC parse error: {e}"),
            Self::Pcap(e) => write!(f, "pcap error: {e}"),
            // TODO: maybe improve this error a bit.
            Self::UnknownDatasetType(dataset_type) => {
                write!(f, "unknown dataset type '{dataset_type}'")
            }
            Self::Dataset(e) => write!(f, "dataset error: {e}"),
        }
    }
}

impl From<MacParseError> for CliError {
    fn from(e: MacParseError) -> Self {
        Self::MacParse(e)
    }
}

impl From<MacAddressError> for CliError {
    fn from(e: MacAddressError) -> Self {
        Self::MacAddress(e)
    }
}

impl From<pcap::Error> for CliError {
    fn from(e: pcap::Error) -> Self {
        Self::Pcap(e)
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
