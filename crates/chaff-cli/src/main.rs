//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

// Not unit-testable
#![cfg(not(tarpaulin_include))]

use std::{path::PathBuf, str::FromStr as _};

use bpaf::Bpaf;
use chaff::framework::Framework;
use chaff_capture::{
    capture::{capture_for, capture_to_trace, find_interface},
    errors::CaptureError,
    trace::Trace,
};
use chaff_cli::errors::CliError;
use chaff_datasets::parsers::tiktok;
use chaff_machines::test::construct_test_machine;
use chaff_sim::Simulator;
use mac_address::{MacAddress, MacAddressError, get_mac_address};
use pcap::Capture;

/// Command-line interface options
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum CliOptions {
    /// Capture a traffic trace.
    #[bpaf(command("capture"))]
    Capture {
        /// Path to output file.
        #[bpaf(short, long)]
        output: Option<PathBuf>,

        /// Name of the interface to use
        #[bpaf(short, long)]
        ifname: Option<String>,
    },

    /// Get statistics about a trace.
    #[bpaf(command("trace-stats"))]
    TraceStats {
        /// Path to trace file.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,
    },

    /// Get statistics about a dataset.
    #[bpaf(command("dataset-stats"))]
    DatasetStats {
        /// The type of dataset.
        ///
        /// Available:
        /// - tiktok
        #[bpaf(positional("TYPE"))]
        dataset_type: String,

        /// The path to the dataset directory.
        #[bpaf(positional("PATH"))]
        path: PathBuf,
    },

    /// Simulate defences.
    #[bpaf(command("sim"))]
    Simulate {
        /// Path to trace file to simulate a machine on.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,
    },

    /// Convert a pcap into a chaff trace.
    #[bpaf(command("convert"))]
    Convert {
        /// Path to input pcap file.
        #[bpaf(positional("PCAP"))]
        pcap: PathBuf,

        /// Path to output trace file.
        #[bpaf(positional("TRACE"))]
        trace: PathBuf,

        /// MAC address to use as the local file.
        #[bpaf(short, long)]
        mac: Option<String>,
    },

    /// Convert a dataset into the chaff trace format.
    #[bpaf(command("dataset-convert"))]
    DatasetConvert {
        /// The type of dataset.
        ///
        /// Available:
        /// - tiktok
        #[bpaf(positional("TYPE"))]
        dataset_type: String,

        /// The path to the dataset directory.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,

        /// The path to output the dataset to. Defaults to <INPUT>.chaff
        #[bpaf(positional("OUTPUT"))]
        output: Option<PathBuf>,
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
        }
        CliOptions::TraceStats { input } => {
            let trace = Trace::deserialise(&input)?;
            println!("Packets: {}", trace.directions.len());
        }
        CliOptions::DatasetStats { dataset_type, path } => {
            let dataset = match dataset_type.to_lowercase().as_str() {
                "tiktok" => tiktok::try_parse(path).map_err(CliError::Dataset),
                other => Err(CliError::UnknownDatasetType(other.to_string())),
            }?;

            println!("Classes: {:?}", dataset.classes());
            println!("Padding to: {}", dataset.get_pad_to());
            println!(
                "Total packets: {}",
                dataset
                    .get_dataset()
                    .iter()
                    .flat_map(|(_, traces)| traces.iter().map(|trace| trace.len() as u64))
                    .sum::<u64>()
            );
        }
        CliOptions::Simulate { input } => {
            let trace = Trace::deserialise(&input)?;
            let machine = construct_test_machine();
            let framework = Framework::new(machine, rand::rng());
            let mut sim: Simulator<_> = Simulator::with(framework, trace, rand::rng());

            println!("{}", sim.run());
            println!("{}", sim.framework.get_state());
        }
        CliOptions::Convert { pcap, trace, mac } => {
            let mac_address = if let Some(mac_string) = mac {
                MacAddress::from_str(mac_string.as_str()).map_err(CliError::MacParse)?
            } else {
                get_mac_address()?.ok_or(CliError::MacAddress(MacAddressError::InternalError))?
            };

            let mut cap = Capture::from_file(&pcap)?;
            let out = capture_to_trace(&mut cap, mac_address)?;
            out.serialise(&trace)?;
        }
        CliOptions::DatasetConvert {
            dataset_type,
            input,
            output,
        } => {
            let output_path =
                output.unwrap_or_else(|| input.clone().as_path().with_extension("chaff"));

            println!(
                "Converting {} to {}...",
                input.display(),
                output_path.display()
            );

            let dataset = match dataset_type.to_lowercase().as_str() {
                "tiktok" => tiktok::try_parse(&input).map_err(CliError::Dataset)?,
                other => return Err(CliError::UnknownDatasetType(other.to_string())),
            };

            std::fs::create_dir_all(&output_path)
                .map_err(|e| CliError::from(chaff_capture::errors::TraceError::Io(e)))?;

            for (class, traces) in dataset.get_dataset() {
                for (i, trace) in traces.iter().enumerate() {
                    let filename = format!("{class}-{i}");
                    let file_path = output_path.join(filename);
                    trace.serialise(&file_path)?;
                }
            }

            println!("Converted dataset to {}", output_path.display());
        }
    }

    Ok(())
}
