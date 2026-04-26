//! Module for the `chaff-cli trace-stats` subcommand.

use std::path::PathBuf;

use chaff_capture::trace::{Direction, Trace};

use crate::errors::CliError;

use iterstats::Iterstats as _;

/// Run the trace stats subcommand with the given trace.
///
/// # Errors
///
/// - If deserialising the trace fails [`Trace::deserialise`].
#[expect(clippy::similar_names)]
pub fn run(input: &PathBuf) -> Result<(), CliError> {
    let trace = Trace::deserialise(&input)?;
    let deltas = trace.timing_deltas();
    let sizes = trace.sizes();
    let total = trace.len();

    let sent = trace
        .directions()
        .iter()
        .filter(|direction| **direction == Direction::Send)
        .count();

    let avg_delta = deltas.iter().map(|delta| f64::from(*delta)).mean();
    let std_delta = deltas.iter().map(|delta| f64::from(*delta)).stddev();

    let avg_size = sizes.iter().map(|size| f64::from(*size)).mean();
    let std_size = sizes.iter().map(|size| f64::from(*size)).stddev();

    let largest_burst_0mus = deltas
        .split(|&delta| delta != 0)
        .map(<[u32]>::len)
        .max()
        .unwrap_or(0);
    let largest_burst_100mus = deltas
        .split(|&delta| delta > 100)
        .map(<[u32]>::len)
        .max()
        .unwrap_or(0);
    let largest_burst_1ms = deltas
        .split(|&delta| delta > 1000)
        .map(<[u32]>::len)
        .max()
        .unwrap_or(0);
    let largest_burst_10ms = deltas
        .split(|&delta| delta > 10000)
        .map(<[u32]>::len)
        .max()
        .unwrap_or(0);
    let largest_burst_100ms = deltas
        .split(|&delta| delta > 100_000)
        .map(<[u32]>::len)
        .max()
        .unwrap_or(0);
    #[expect(clippy::cast_sign_loss)]
    #[expect(clippy::cast_possible_truncation)]
    let largest_burst_avg = deltas
        .split(|&delta| delta > avg_delta as u32)
        .map(<[u32]>::len)
        .max()
        .unwrap_or(0);
    let largest_delta = deltas.iter().max().unwrap_or(&0);

    println!("Packets: {total}");
    println!("Sent: {sent}");
    println!("Received: {}", total - sent);
    println!("Average packet size: {avg_size:.2}±{std_size:.2} bytes");
    println!("Average time delta: {avg_delta:.2}±{std_delta:.2}μs");
    println!("Largest time delta: {largest_delta}μs");
    println!("Largest burst (0μs): {largest_burst_0mus} packets");
    println!("Largest burst (100μs): {largest_burst_100mus} packets");
    println!("Largest burst (1ms): {largest_burst_1ms} packets");
    println!("Largest burst (10ms): {largest_burst_10ms} packets");
    println!("Largest burst (100ms): {largest_burst_100ms} packets");
    println!("Largest burst (average delta): {largest_burst_avg} packets");
    Ok(())
}
