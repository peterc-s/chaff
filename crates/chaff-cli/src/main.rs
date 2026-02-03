//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

use std::{error::Error, fmt};

use bpaf::Bpaf;
use chaff::capture::{capture_for_ms, find_interface};

/// Command-line interface options
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum CliOptions {
    #[bpaf(command("capture"))]
    /// Capture a traffic trace
    Capture {
        /// Name of the interface to use
        #[bpaf(short, long)]
        ifname: Option<String>,
    },
}

/// CLI errors
#[derive(Debug)]
pub enum CliError {
    /// Device specified but not found.
    DeviceNotFound(String),
}

impl Error for CliError {}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DeviceNotFound(name) => write!(f, "Device {name} specified but not found."),
        }
    }
}

/// Dummy docs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_opts = cli_options().run();

    // Match for subcommands
    match cli_opts {
        CliOptions::Capture { ifname } => {
            let device = match ifname {
                Some(name) => {
                    println!("Searching for device {name}...");
                    find_interface(&name)?.map_or_else(
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
            let cap = capture_for_ms(std::time::Duration::from_secs(10), device)?;
            println!("Captured {} packets.", cap.directions.len());
        }
    }

    Ok(())
}
