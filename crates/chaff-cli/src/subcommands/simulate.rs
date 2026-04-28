//! Module for the `chaff-cli sim` subcommand.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use chaff::{framework::Framework, machine::Machine};
use chaff_capture::trace::Trace;
use chaff_datasets::dataset::{Dataset, DatasetBuilder};
use chaff_sim::{Simulator, SimulatorOverheads};

use crate::errors::CliError;

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

    println!("{trace}");
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
///
/// # Panics
///
/// If a [`std::sync::Mutex::lock`] fails.
pub fn run_dataset(
    dataset: &Dataset,
    output: &Option<PathBuf>,
    machine: &Machine,
) -> Result<(), CliError> {
    let input_data = dataset.get_dataset();
    let mut output_dataset_builder = DatasetBuilder::new(dataset.get_pad_to());

    let tasks: Vec<_> = input_data
        .iter()
        .flat_map(|(class, traces)| traces.iter().map(move |trace| (class, trace)))
        .collect();

    let num_tasks = tasks.len();
    let num_threads = thread::available_parallelism().map_or(1, std::num::NonZero::get);

    let work_queue = Arc::new(Mutex::new(tasks.into_iter()));
    let (tx, rx) = mpsc::channel();

    thread::scope(|s| {
        for _ in 0..num_threads {
            let thread_tx = tx.clone();
            let thread_machine = machine.clone();
            let thread_queue = Arc::clone(&work_queue);

            s.spawn(move || {
                let mut sim = Simulator::with(
                    Framework::new(thread_machine, rand::rng()),
                    Trace::default(),
                    rand::rng(),
                );

                loop {
                    let task = {
                        #[expect(clippy::expect_used)]
                        let mut queue = thread_queue
                            .lock()
                            .expect("other thread panicked while holding thread queue");
                        queue.next()
                    };

                    match task {
                        Some((class, trace)) => {
                            sim.replace_trace(trace.clone());
                            let (out_trace, overhead) = sim.run();
                            let _ = thread_tx.send((class, out_trace, overhead));
                        }
                        None => break,
                    }
                }
            });
        }

        drop(tx);

        let mut overheads = Vec::with_capacity(num_tasks);
        while let Ok((class, out_trace, overhead)) = rx.recv() {
            output_dataset_builder.push_to_class(class, out_trace);
            overheads.push(overhead);
        }

        let overheads_total = SimulatorOverheads::total_from(overheads);
        if let Some(total) = overheads_total {
            println!("{total}");
        }
    });

    if let Some(output) = output {
        if !output.exists() {
            fs::create_dir_all(output)?;
        }

        output_dataset_builder.build().dump_to(output)?;
    }

    Ok(())
}
