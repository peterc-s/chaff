//! Test using pcap

use chaff::capture::capture_for_ms;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting capture");
    let trace = capture_for_ms(std::time::Duration::from_secs(10))?;
    println!("{trace:?}");

    let first_packet = (
        &trace.directions[0],
        &trace.timing_deltas[0],
        &trace.sizes[0],
    );
    println!("First packet: {first_packet:?}");
    Ok(())
}
