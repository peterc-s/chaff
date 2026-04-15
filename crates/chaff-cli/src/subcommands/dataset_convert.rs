//! Module for the `chaff-cli dataset-convert` subcommand.

use std::{fs, path::PathBuf};

use crate::{errors::CliError, utils::parse_dataset};

/// Runs the dataset conversion subcommand.
///
/// # Errors
///
/// - If parsing the dataset fails ([`parse_dataset`] and the parser's `try_parse`, for example [`chaff_datasets::parsers::tiktok::try_parse`]).
/// - If creating directories fails ([`fs::create_dir_all`]).
/// - If serialising traces fails ([`chaff_capture::trace::Trace::serialise`]).
pub fn run(output: Option<PathBuf>, dataset_type: &str, input: &PathBuf) -> Result<(), CliError> {
    let output_path = output.unwrap_or_else(|| input.clone().as_path().with_extension("chaff"));

    println!(
        "Converting {} to {}...",
        input.display(),
        output_path.display()
    );

    let dataset = parse_dataset(dataset_type, input)?;

    fs::create_dir_all(&output_path).map_err(CliError::Io)?;

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
