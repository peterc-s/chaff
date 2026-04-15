//! A collection of helper methods and single source of truth methods for use across chaff-cli

use std::path::PathBuf;

use chaff_datasets::{dataset::Dataset, parsers::tiktok};

use crate::errors::CliError;

/// Parse a dataset with the given type and path.
///
/// # Errors
///
/// - If the dataset type is not one of the accepted types.
/// - If the parsers `try_from` fails (see [`chaff_datasets::parsers`] for a list of parsers).
pub fn parse_dataset(dataset_type: &str, input: &PathBuf) -> Result<Dataset, CliError> {
    match dataset_type.to_lowercase().as_str() {
        "tiktok" => Ok(tiktok::try_parse(input).map_err(CliError::Dataset)?),
        other => Err(CliError::UnknownDatasetType(other.to_string())),
    }
}
