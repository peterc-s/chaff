//! Module for the `chaff-cli sim` subcommand.

use std::{fs, path::PathBuf};

use chaff::{framework::Framework, machine::Machine};
use chaff_capture::trace::Trace;
use chaff_datasets::dataset::DatasetBuilder;
use chaff_sim::{Simulator, SimulatorOverheads};

use crate::{errors::CliError, utils::parse_dataset};

/// Run the simulator on a singular trace.
///
/// # Errors
///
/// If deserialising the given trace file fails. If output is supplied, an error may be returned if
/// serialising fails.
pub fn run_trace(
    input: &PathBuf,
    output: &Option<PathBuf>,
    machine: Machine,
) -> Result<(), CliError> {
    let trace = Trace::deserialise(input)?;
    let framework = Framework::new(machine, rand::rng());
    let mut sim: Simulator<_> = Simulator::with(framework, trace, rand::rng());
    let (trace, overheads) = sim.run();

    // println!("{trace}");
    println!("Final state: {}", sim.framework.get_state());
    println!("{overheads}");

    if let Some(output) = output {
        trace.serialise(output)?;
    }

    Ok(())
}

/// Run the simulator on a dataset.
///
/// # Errors
///
/// If parsing the dataset fails. If output is supplied, an error may be returned if dumping fails.
pub fn run_dataset(
    input: &PathBuf,
    dataset_type: &str,
    output: &Option<PathBuf>,
    machine: Machine,
) -> Result<(), CliError> {
    let input_dataset = parse_dataset(dataset_type, input)?;
    let input_data = input_dataset.get_dataset();

    let mut output_dataset_builder = DatasetBuilder::new(input_dataset.get_pad_to());

    let framework = Framework::new(machine, rand::rng());
    let mut sim = Simulator::with(framework, Trace::default(), rand::rng());
    let mut overheads = Vec::with_capacity(input_data.len());

    for (class, traces) in input_data {
        for trace in traces {
            sim.replace_trace(trace.clone());
            let (trace, overhead) = sim.run();
            output_dataset_builder.push_to_class(class, trace);
            overheads.push(overhead);
        }
    }

    let overheads = SimulatorOverheads::total_from(overheads);
    if let Some(overheads) = overheads {
        println!("{overheads}");
    }

    if let Some(output) = output {
        if !output.exists() {
            fs::create_dir_all(output)?;
        }

        output_dataset_builder.build().dump_to(output)?;
    }

    Ok(())
}
