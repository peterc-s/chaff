//! Module for the `chaff-cli sim` subcommand.

use std::path::PathBuf;

use chaff::framework::Framework;
use chaff_capture::trace::Trace;
use chaff_machines::test::construct_test_machine;
use chaff_sim::Simulator;

use crate::errors::CliError;

/// Run the simulaor subcommand.
///
/// # Errors
///
/// - If deserialising the trace fails [`Trace::deserialise`].
pub fn run(input: &PathBuf) -> Result<(), CliError> {
    let trace = Trace::deserialise(input)?;
    let machine = construct_test_machine();
    let framework = Framework::new(machine, rand::rng());
    let mut sim: Simulator<_> = Simulator::with(framework, trace, rand::rng());

    println!("{}", sim.run());
    println!("{}", sim.framework.get_state());

    Ok(())
}
