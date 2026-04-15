//! Module for the `chaff-cli trace-stats` subcommand.

use std::path::PathBuf;

use chaff_capture::trace::Trace;

use crate::errors::CliError;

/// Run the trace stats subcommand with the given trace.
pub fn run(input: &PathBuf) -> Result<(), CliError> {
    let trace = Trace::deserialise(&input)?;
    println!("Packets: {}", trace.directions.len());
    Ok(())
}
