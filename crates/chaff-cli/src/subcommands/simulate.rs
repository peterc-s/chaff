//! Module for the `chaff-cli sim` subcommand.

use std::{fs, path::PathBuf};

use chaff::framework::Framework;
use chaff_capture::trace::Trace;
use chaff_datasets::dataset::DatasetBuilder;
use chaff_machines::test::construct_test_machine;
use chaff_sim::Simulator;

use crate::{errors::CliError, utils::parse_dataset};

/// Run the simulator on a singular trace.
///
/// # Errors
///
/// If deserialising the given trace file fails. If output is supplied, an error may be returned if
/// serialising fails.
pub fn run_trace(input: &PathBuf, output: &Option<PathBuf>) -> Result<(), CliError> {
    let trace = Trace::deserialise(input)?;
    let machine = construct_test_machine();
    let framework = Framework::new(machine, rand::rng());
    let mut sim: Simulator<_> = Simulator::with(framework, trace, rand::rng());
    let out = sim.run();

    println!("{out}");
    println!("Final state: {}", sim.framework.get_state());

    if let Some(output) = output {
        out.serialise(output)?;
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
) -> Result<(), CliError> {
    let input_dataset = parse_dataset(dataset_type, input)?;
    let input_data = input_dataset.get_dataset();

    let mut output_dataset_builder = DatasetBuilder::new(input_dataset.get_pad_to());

    let machine = construct_test_machine();
    let framework = Framework::new(machine, rand::rng());
    let mut sim = Simulator::with(framework, Trace::default(), rand::rng());

    for (class, traces) in input_data {
        for trace in traces {
            sim.replace_trace(trace.clone());
            let trace = sim.run();
            output_dataset_builder.push_to_class(class, trace);
        }
    }

    if let Some(output) = output {
        if !output.exists() {
            fs::create_dir_all(output)?;
        }

        output_dataset_builder.build().dump_to(output)?;
    }

    Ok(())
}
