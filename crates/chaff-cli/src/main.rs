//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

// Not currently tested.
#![cfg(not(tarpaulin_include))]

use borsh::BorshDeserialize as _;
use std::{fs::File, path::PathBuf};

use bpaf::Bpaf;
use chaff::machine::Machine;
use chaff_cli::{
    errors::CliError,
    subcommands::{cap_convert, capture, dataset_convert, dataset_stats, simulate, trace_stats},
    utils::parse_dataset,
};
use chaff_machines::constant;

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
        /// The type of dataset (available: chaff, tiktok).
        #[bpaf(positional("TYPE"))]
        dataset_type: String,

        /// The path to the dataset directory.
        #[bpaf(positional("PATH"))]
        path: PathBuf,
    },

    /// Simulate defences.
    #[bpaf(command("sim"))]
    Simulate {
        /// A path to the serialised machine to use.
        #[bpaf(short, long)]
        machine: Option<PathBuf>,

        /// The simulator subcommand to take.
        #[bpaf(external(sim))]
        action: Sim,
    },

    /// Convert a pcap into a chaff trace.
    #[bpaf(command("cap-convert"))]
    CapConvert {
        /// MAC address to use as the local file.
        #[bpaf(short, long)]
        mac: Option<String>,

        /// Path to input pcap file.
        #[bpaf(positional("PCAP"))]
        pcap: PathBuf,

        /// Path to output trace file.
        #[bpaf(positional("TRACE"))]
        trace: PathBuf,
    },

    /// Convert a dataset into the chaff trace format.
    #[bpaf(command("dataset-convert"))]
    DatasetConvert {
        /// The path to output the dataset to. Defaults to <INPUT>.chaff
        #[bpaf(short, long)]
        output: Option<PathBuf>,

        /// The type of dataset (available: chaff, tiktok).
        #[bpaf(positional("TYPE"))]
        dataset_type: String,

        /// The path to the dataset directory.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,
    },
}

/// Available simulation modes:
#[derive(Debug, Clone, Bpaf)]
pub enum Sim {
    /// Simulate on a trace file.
    #[bpaf(command("trace"))]
    Trace {
        /// Path to output trace file.
        #[bpaf(short, long)]
        output: Option<PathBuf>,

        /// Path to input trace file.
        #[bpaf(positional("INPUT"))]
        input: PathBuf,
    },

    /// Simulate on a full dataset.
    #[bpaf(command("dataset"))]
    Dataset {
        /// Path to output dataset directory.
        #[bpaf(short, long)]
        output: Option<PathBuf>,

        /// The type of dataset (available: chaff, tiktok).
        #[bpaf(positional("TYPE"))]
        dataset_type: String,

        /// Path to input dataset directory.
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
        CliOptions::Capture { output, ifname } => capture::run(output, ifname),
        CliOptions::TraceStats { input } => trace_stats::run(&input),
        CliOptions::DatasetStats { dataset_type, path } => dataset_stats::run(&dataset_type, &path),
        CliOptions::Simulate { action, machine } => {
            let machine = if let Some(path) = machine {
                let mut file = File::open(path)?;
                Machine::deserialize_reader(&mut file)?
            } else {
                constant::construct()
            };

            match action {
                Sim::Trace { output, input } => simulate::run_trace(&input, &output, machine),
                Sim::Dataset {
                    output,
                    input,
                    dataset_type,
                } => {
                    let input_dataset = parse_dataset(&dataset_type, &input)?;
                    simulate::run_dataset(&input_dataset, &output, &machine)
                }
            }
        }
        CliOptions::CapConvert { pcap, trace, mac } => cap_convert::run(mac, &pcap, &trace),
        CliOptions::DatasetConvert {
            dataset_type,
            input,
            output,
        } => dataset_convert::run(output, &dataset_type, &input),
    }
}
