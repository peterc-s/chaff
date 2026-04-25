//! Parsing logic for Chaff format datasets.
//!
//! Chaff format datasets are a collection of Chaff [`Trace`] format files which are named as such:
//! `<class_name>-<instance_num>`. Files are deserialised via [`Trace::deserialise`].

use std::{collections::HashMap, fs, path::Path};

use chaff_capture::trace::Trace;

use crate::{dataset::Dataset, errors::DatasetError};

/// Parse a Chaff foramat dataset into a [`Dataset`].
///
/// # Errors
///
/// May error for one of the following reasons:
///
/// - This function expects to be passed a path to a directory.
/// - This function expects file names in that directory to follow the format noted [here](crate::parsers::chaff).
/// - This function expects traces within those files to follow the format also noted [here](crate::parsers::chaff).
/// - Any [`std::io::Error`]s will be propagated.
pub fn try_parse<P: AsRef<Path>>(path: P) -> Result<Dataset, DatasetError> {
    let path = path.as_ref();

    if !path.is_dir() {
        return Err(DatasetError::NotADirectory(path.to_path_buf()));
    }

    let mut data_with_instance: HashMap<String, Vec<(usize, Trace)>> = HashMap::default();

    for entry in fs::read_dir(path).map_err(DatasetError::Io)? {
        let entry = entry.map_err(DatasetError::Io)?;
        let file_type = entry.file_type().map_err(DatasetError::Io)?;

        // don't follow symlinks or subdirectories
        if !file_type.is_file() {
            continue;
        }

        let file_name_os = entry.file_name();
        let file_name = file_name_os.to_string_lossy();

        let parts: Vec<&str> = file_name.split('-').collect();

        // skip invalid filename format.
        if parts.len() != 2 {
            continue;
        }

        let class = parts[0].to_string();
        let instance: usize = parts[1]
            .parse()
            .map_err(|_| DatasetError::InvalidFileName {
                file: entry.path(),
                message: "expected two unsigned integers separated by hyphens".to_string(),
            })?;

        let trace = Trace::deserialise(&entry.path()).map_err(DatasetError::ChaffSerDe)?;

        data_with_instance
            .entry(class)
            .or_default()
            .push((instance, trace));
    }

    let mut data = HashMap::new();

    for (class, mut instances) in data_with_instance {
        instances.sort_by_key(|&(instance, _)| instance);
        data.insert(
            class,
            instances.into_iter().map(|(_, trace)| trace).collect(),
        );
    }

    Ok(Dataset { data, pad_to: 0 })
}
