//! Module for the `chaff-cli cap-convert` subcommand.

use std::{path::PathBuf, str::FromStr as _};

use chaff_capture::capture::capture_to_trace;
use mac_address::{MacAddress, MacAddressError, get_mac_address};
use pcap::Capture;

use crate::errors::CliError;

/// Run the pcap conversion subcommand.
///
/// # Errors
///
/// - If parsing the given MAC address fails ([`MacAddress::from_str`]).
/// - If a MAC wasn't given and we weren't successful in getting a MAC address from the system ([`get_mac_address`]).
/// - If parsing or opening the pcap fails ([`Capture::from_file`]).
/// - If converting the pcap to a trace fails ([`capture_to_trace`]).
/// - If serialising the trace fails ([`chaff_capture::trace::Trace::serialise`]).
pub fn run(mac: Option<String>, pcap: &PathBuf, trace: &PathBuf) -> Result<(), CliError> {
    let mac_address = if let Some(mac_string) = mac {
        MacAddress::from_str(mac_string.as_str()).map_err(CliError::MacParse)?
    } else {
        get_mac_address()?.ok_or(CliError::MacAddress(MacAddressError::InternalError))?
    };

    let mut cap = Capture::from_file(pcap)?;
    let out = capture_to_trace(&mut cap, mac_address)?;
    out.serialise(trace)?;

    Ok(())
}
