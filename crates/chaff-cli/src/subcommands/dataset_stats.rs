//! Module for the `chaff-cli dataset-stats` subcommand.

use std::path::PathBuf;

use chaff_datasets::parsers::tiktok;

use crate::errors::CliError;

/// Run the dataset-stats subcommand.
pub fn run(dataset_type: &str, path: PathBuf) -> Result<(), CliError> {
    let dataset = match dataset_type.to_lowercase().as_str() {
        "tiktok" => tiktok::try_parse(path).map_err(CliError::Dataset),
        other => Err(CliError::UnknownDatasetType(other.to_string())),
    }?;

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
