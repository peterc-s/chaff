//! Module for the `chaff-cli dataset-convert` subcommand.

use std::path::PathBuf;

use chaff_datasets::parsers::tiktok;

use crate::errors::CliError;

/// Runs the dataset conversion subcommand.
pub fn run(output: Option<PathBuf>, dataset_type: &str, input: &PathBuf) -> Result<(), CliError> {
    let output_path = output.unwrap_or_else(|| input.clone().as_path().with_extension("chaff"));

    println!(
        "Converting {} to {}...",
        input.display(),
        output_path.display()
    );

    let dataset = match dataset_type.to_lowercase().as_str() {
        "tiktok" => tiktok::try_parse(input).map_err(CliError::Dataset)?,
        other => return Err(CliError::UnknownDatasetType(other.to_string())),
    };

    std::fs::create_dir_all(&output_path)
        .map_err(|e| CliError::from(chaff_capture::errors::TraceError::Io(e)))?;

    for (class, traces) in dataset.get_dataset() {
        for (i, trace) in traces.iter().enumerate() {
            let filename = format!("{class}-{i}");
            let file_path = output_path.join(filename);
            trace.serialise(&file_path)?;
        }
    }

    println!("Converted dataset to {}", output_path.display());

    Ok(())
}
