//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

// Not unit-testable
#![cfg(not(tarpaulin_include))]

use std::path::PathBuf;

use bpaf::Bpaf;
use chaff::{
    capture::{capture_for, find_interface},
    errors::{CaptureError, ChaffError},
    framework::Framework,
    trace::Trace,
};
use chaff_cli::errors::CliError;
use chaff_machines::test::construct_test_machine;
use chaff_sim::Simulator;

/// Command-line interface options
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum CliOptions {
    #[bpaf(command("capture"))]
    /// Capture a traffic trace.
    Capture {
        /// Path to output file.
        #[bpaf(short, long)]
        output: Option<PathBuf>,

        /// Name of the interface to use
        #[bpaf(short, long)]
        ifname: Option<String>,
    },

    #[bpaf(command("trace-stats"))]
    /// Get statistics about a trace.
    TraceStats {
        /// Path to trace file.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,
    },

    #[bpaf(command("sim"))]
    /// Simulate defences.
    Simulate {
        /// Path to trace file to simulate a machine on.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,
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
        CliOptions::Capture { output, ifname } => {
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

            if let Some(path) = output {
                cap.serialise(&path)?;
                println!("Saved to {}", path.display());
            }
        }
        CliOptions::TraceStats { input } => {
            let trace = Trace::deserialise(&input)?;
            println!("Packets: {}", trace.directions.len());
        }
        CliOptions::Simulate { input } => {
            let trace = Trace::deserialise(&input)?;
            let machine = construct_test_machine();
            let framework = Framework::new(machine, rand::rng());
            let mut sim: Simulator<_> = Simulator::with(framework, trace);

            println!("{}", sim.run());
            println!("{}", sim.framework.get_state());
        }
    }

    Ok(())
}
