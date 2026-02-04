//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

use bpaf::Bpaf;
use chaff::{
    capture::{capture_for, find_interface},
    errors::{CaptureError, ChaffError},
};
use chaff_cli::errors::CliError;

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

/// Wrapper around [`run()`] with error printing.
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

/// Run the chaff CLI.
fn run() -> Result<(), CliError> {
    let cli_opts = cli_options().run();

    // Match for subcommands
    match cli_opts {
        CliOptions::Capture { ifname } => {
            let device = match ifname {
                Some(name) => {
                    println!("Searching for device {name}...");
                    find_interface(&name)
                        .map_err(CaptureError::Pcap)
                        .map_err(ChaffError::Capture)?
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
            let cap = capture_for(std::time::Duration::from_secs(10), device)
                .map_err(ChaffError::Capture)?;
            println!("Captured {} packets.", cap.directions.len());
        }
    }

    Ok(())
}
