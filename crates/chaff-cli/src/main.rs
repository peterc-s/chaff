//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

// Not unit-testable
#![cfg(not(tarpaulin_include))]

use std::path::PathBuf;

use bpaf::Bpaf;
use chaff_cli::{
    errors::CliError,
    subcommands::{cap_convert, capture, dataset_convert, dataset_stats, simulate, trace_stats},
};

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
        /// The type of dataset (available: tiktok).
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

        /// The type of dataset (available: tiktok).
        #[bpaf(positional("TYPE"))]
        dataset_type: String,

        /// The path to the dataset directory.
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
        CliOptions::Simulate { input } => simulate::run(&input),
        CliOptions::CapConvert { pcap, trace, mac } => cap_convert::run(mac, &pcap, &trace),
        CliOptions::DatasetConvert {
            dataset_type,
            input,
            output,
        } => dataset_convert::run(output, &dataset_type, &input),
    }
}
