//! Module for the `chaff-cli capture` subcommand.

use std::path::PathBuf;

use chaff_capture::{
    capture::{capture_for, find_interface},
    errors::CaptureError,
};

use crate::errors::CliError;

/// Run the capture subcommand with given output and ifname.
pub fn run(output: Option<PathBuf>, ifname: Option<String>) -> Result<(), CliError> {
    let device = match ifname {
        Some(name) => {
            println!("Searching for device {name}...");
            find_interface(&name)
                .map_err(CaptureError::from)?
                .map_or_else(
                    || {
                        println!("Device {name} not found.");
                        Err(CliError::DeviceNotFound(name))
                    },
                    |device| {
                        println!("Device found.");
                        Ok(Some(device))
                    },
                )
        }
        None => Ok(None),
    }?;
    let cap = capture_for(std::time::Duration::from_secs(10), device)?;

    println!("Captured {} packets.", cap.directions.len());

    if let Some(path) = output {
        cap.serialise(&path)?;
        println!("Saved to {}", path.display());
    }

    Ok(())
}
