//! Parsing logic for [Tik-Tok](https://github.com/msrocean/Tik_Tok)
//!
//! This implements parsing from their Directional Timing (DT) representation. The dataset the authors
//! give is a collection of text files which are named as such: `<class_num>-<instance_num>`.
//!
//! Each file in DT representation consists of as many lines as packets in the trace, each in the
//! following format: `<timestamp (s)> <directional size>`.

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead as _, BufReader},
    path::Path,
};

use chaff_capture::trace::{Direction, Trace, TraceBuilder};

use crate::{dataset::Dataset, errors::ParseError};

/// Parse a [Tik-Tok](https://github.com/msrocean/Tik_Tok) dataset into a [`Dataset`].
///
/// Will pad to `5000` with `0`s.
///
/// # Errors
///
/// May error for one of the following reasons:
///
/// - This function expects to be passed a path to a directory.
/// - This function expects file names in that directory to follow the format noted
///   [here](crate::parsers::tiktok).
/// - This function expects traces within those files to follow the format also noted
///   [here](crate::parsers::tiktok).
/// - Any [`std::io::Error`]s will be propagated.
pub fn try_parse<P: AsRef<Path>>(path: P) -> Result<Dataset, ParseError> {
    let path = path.as_ref();

    if !path.is_dir() {
        return Err(ParseError::NotADirectory(path.to_path_buf()));
    }

    let mut data_with_instance: HashMap<String, Vec<(usize, Trace)>> = HashMap::default();

    for entry in fs::read_dir(path).map_err(ParseError::Io)? {
        let entry = entry.map_err(ParseError::Io)?;
        let file_type = entry.file_type().map_err(ParseError::Io)?;

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
        let instance: usize = parts[1].parse().map_err(|_| ParseError::InvalidFileName {
            file: entry.path(),
            message: "expected two unsigned integers separated by hyphens".to_string(),
        })?;

        let file = File::open(entry.path()).map_err(ParseError::Io)?;
        let reader = BufReader::new(file);

        let mut trace_builder: Option<TraceBuilder> = None;

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(ParseError::Io)?;
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() != 2 {
                return Err(ParseError::InvalidFormat {
                    file: entry.path(),
                    line: line_num + 1,
                    message: "expected: <timestamp> <directional size>, but didn't get two parts"
                        .to_string(),
                });
            }

            let timestamp_sec: f64 = tokens[0].parse().map_err(|_| ParseError::InvalidFormat {
                file: entry.path(),
                line: line_num + 1,
                message: format!("invalid timestamp: {}", tokens[0]),
            })?;

            let dir_size: i32 = tokens[1].parse().map_err(|_| ParseError::InvalidFormat {
                file: entry.path(),
                line: line_num + 1,
                message: format!("invalid directional size: {}", tokens[1]),
            })?;

            #[expect(clippy::cast_sign_loss)]
            #[expect(clippy::cast_possible_truncation)]
            let timestamp_microsec: u64 = (timestamp_sec * 1_000_000.0) as u64;

            let size = dir_size.unsigned_abs();
            let direction = if dir_size > 0 {
                Direction::Send
            } else {
                Direction::Receive
            };

            trace_builder
                .get_or_insert_with(TraceBuilder::default)
                .record(direction, timestamp_microsec, size);
        }

        if let Some(builder) = trace_builder {
            data_with_instance
                .entry(class)
                .or_default()
                .push((instance, builder.build()));
        }
    }

    let mut data = HashMap::new();

    for (class, mut instances) in data_with_instance {
        instances.sort_by_key(|&(instance, _)| instance);
        data.insert(
            class,
            instances.into_iter().map(|(_, trace)| trace).collect(),
        );
    }

    // 5000 is the padding documented by the Tik-Tok authors.
    Ok(Dataset { data, pad_to: 5000 })
}
