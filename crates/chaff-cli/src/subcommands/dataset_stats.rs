//! Module for the `chaff-cli dataset-stats` subcommand.

use std::path::PathBuf;

use crate::{errors::CliError, utils::parse_dataset};

/// Run the dataset-stats subcommand.
///
/// # Errors
///
/// - If parsing the dataset fails ([`parse_dataset`] and the parser's `try_parse`, for example [`chaff_datasets::parsers::tiktok::try_parse`]).
pub fn run(dataset_type: &str, path: &PathBuf) -> Result<(), CliError> {
    let dataset = parse_dataset(dataset_type, path)?;

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

    Ok(())
}
