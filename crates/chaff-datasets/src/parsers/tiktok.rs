//! Parsing logic for [Tik-Tok](https://github.com/msrocean/Tik_Tok)
//!
//! This implements parsing from their Directional Timing (DT) representation. The dataset the authors
//! give is a collection of text files which are named as such: `<class_num>-<instance_num>`.
//!
//! Each file in DT representation consists of as many lines as packets in the trace, each in the
//! following format: `<timestamp (s)> <directional size>`.

use std::{collections::HashMap, fs, path::Path};

use chaff_capture::trace::{Direction, Trace, TraceBuilder};

use crate::{dataset::Dataset, errors::ParseError};

/// Parse a full line. This function makes a few assumptions about the dataset and does not
/// handle errors very gracefully.
fn parse_line(line: &str) -> Option<(u64, i32)> {
    let bytes = line.as_bytes();
    let mut idx = 0;
    let len = bytes.len();

    // skip whitespace
    while idx < len && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }

    // parse integer part of timestamp
    let mut secs = 0u64;
    while idx < len && bytes[idx].is_ascii_digit() {
        secs = secs * 10 + u64::from(bytes[idx] - b'0');
        idx += 1;
    }

    let mut microsecs = secs * 1_000_000;

    // parse fractional part of timestamp
    if idx < len && bytes[idx] == b'.' {
        idx += 1;
        let mut fraction = 0u64;
        let mut digits = 0;

        while idx < len && bytes[idx].is_ascii_digit() {
            if digits < 6 {
                fraction = fraction * 10 + u64::from(bytes[idx] - b'0');
                digits += 1;
            }
            idx += 1;
        }

        while digits < 6 {
            fraction *= 10;
            digits += 1;
        }

        microsecs += fraction;
    }

    // skip whitespace between timestamp and directional size
    while idx < len && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }

    // missing directional size
    if idx == len {
        return None;
    }

    // parse directional size
    let mut dir_size = 0i32;
    let mut is_negative = false;

    if bytes[idx] == b'-' {
        is_negative = true;
        idx += 1;
    }

    while idx < len && bytes[idx].is_ascii_digit() {
        dir_size = dir_size * 10 + i32::from(bytes[idx] - b'0');
        idx += 1;
    }

    if is_negative {
        dir_size = -dir_size;
    }

    Some((microsecs, dir_size))
}

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
///
/// This function may also silently fail due to the parsing logic. The internal line parsing
/// function has been made for performance. If you are giving the dataset as described
/// [here](crate::parsers::tiktok), this should not be an issue.
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

        let content = fs::read_to_string(entry.path()).map_err(ParseError::Io)?;
        let mut trace_builder: Option<TraceBuilder> = None;

        for (line_num, line) in content.lines().enumerate() {
            if line.is_empty() {
                continue;
            }

            let (timestamp_microsec, dir_size) =
                parse_line(line).ok_or_else(|| ParseError::InvalidFormat {
                    file: entry.path(),
                    line: line_num + 1,
                    message: format!("invalid line format: {line}"),
                })?;

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
