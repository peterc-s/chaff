//! Test using pcap

use chaff_capture::capture::capture_for;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting capture");
    let trace = capture_for(std::time::Duration::from_secs(10), None)?;
    println!("Got {} packets.", trace.len());

    let first_packet = trace.iter().next();
    println!("First packet: {first_packet:?}");

    let last_packet = trace.iter().last();
    println!("Last packet: {last_packet:?}");
    Ok(())
}
