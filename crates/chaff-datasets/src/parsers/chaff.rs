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

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::{fs, path::PathBuf};

    use chaff_capture::trace::{Direction, Trace};
    use tempfile::tempdir;

    use crate::{errors::DatasetError, parsers::chaff};

    #[test]
    fn test_try_parse_rejects_not_a_directory() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-datasets/tiktok/chaff/35-0");

        let err = chaff::try_parse(&path).unwrap_err();

        match err {
            DatasetError::NotADirectory(p) => assert_eq!(p, path),
            other => panic!("unexpected result: {other}"),
        }
    }

    #[test]
    fn test_try_parse_skips_invalid_filenames() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("badname"), "0.000001 1\n").unwrap();
        let trace = Trace::new([Direction::Send, Direction::Receive], [0, 1], [1, 1]);
        trace.serialise(&dir.join("1-0")).unwrap();

        let dataset = chaff::try_parse(dir).unwrap();

        let traces = dataset
            .data
            .get("1")
            .expect("expected class '1' in dataset");
        assert_eq!(traces.len(), 1);

        let t = &traces[0];
        assert_eq!(t.len(), 2);
        let directions = t.directions();
        assert_eq!(directions[0], Direction::Send);
        assert_eq!(directions[1], Direction::Receive);
    }

    #[test]
    fn test_try_parse_sorts_instances_by_instance_number() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        let trace = Trace::new([Direction::Receive], [0], [1]);
        trace.serialise(&dir.join("9-0")).unwrap();
        let trace = Trace::new([Direction::Send], [0], [1]);
        trace.serialise(&dir.join("9-1")).unwrap();

        let dataset = chaff::try_parse(dir).unwrap();
        let traces = dataset.data.get("9").unwrap();
        assert_eq!(traces.len(), 2);

        assert_eq!(traces[0].directions()[0], Direction::Receive);
        assert_eq!(traces[1].directions()[0], Direction::Send);
    }

    #[test]
    fn test_try_parse_skips_subdirectory_entries() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();
        let trace = Trace::new([Direction::Send], [0], [1]);
        trace.serialise(&dir.join("2-0")).unwrap();

        let sub = dir.join("subdir");
        fs::create_dir_all(&sub).unwrap();
        let trace = Trace::new([Direction::Send, Direction::Receive], [0, 1], [1, 1]);
        trace.serialise(&sub.join("1-0")).unwrap();

        let dataset = chaff::try_parse(&tmp).unwrap();

        assert!(dataset.data.contains_key("2"));
        assert!(!dataset.data.contains_key("1"));

        let traces = &dataset.data["2"];
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].len(), 1);
        assert_eq!(traces[0].directions()[0], Direction::Send);
    }

    #[test]
    fn test_try_parse_invalid_instance_number_errors() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("1-notanumber"), "").unwrap();

        let err = chaff::try_parse(&tmp).unwrap_err();
        match err {
            DatasetError::InvalidFileName { file, message } => {
                assert!(file.ends_with("1-notanumber"));
                assert!(message.contains("expected two unsigned integers"));
            }
            other => panic!("unexpected result: {other}"),
        }
    }
}
