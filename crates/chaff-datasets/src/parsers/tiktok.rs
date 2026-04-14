//! Parsing logic for [Tik-Tok](https://github.com/msrocean/Tik_Tok)
//!
//! This implements parsing from their Directional Timing (DT) representation. The dataset the authors
//! give is a collection of text files which are named as such: `<class_num>-<instance_num>`.
//!
//! Each file in DT representation consists of as many lines as packets in the trace, each in the
//! following format: `<timestamp (s)> <directional size>`.

use std::{collections::HashMap, fs, io::Read as _, path::Path};

use chaff_capture::trace::{Direction, Trace, TraceBuilder};

use crate::{dataset::Dataset, errors::ParseError};

/// Parse a full line. This function makes a few assumptions about the dataset and does not
/// handle errors very gracefully.
fn parse_line(line: &str) -> Option<(u64, i32)> {
    // trim whitespace safely
    let line = line.trim();
    let bytes = line.as_bytes();

    // find the space between the two numbers
    let space_idx = bytes.iter().position(|&b| b.is_ascii_whitespace())?;

    // parse timestamp
    let ts_str = &line[..space_idx];
    let timestamp_sec: f64 = ts_str.parse().ok()?;

    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let timestamp_microsec = (timestamp_sec * 1_000_000.0) as u64;

    // find the start of the directional size
    let mut dir_start = space_idx;
    while dir_start < bytes.len() && bytes[dir_start].is_ascii_whitespace() {
        dir_start += 1;
    }

    let mut dir_bytes = &bytes[dir_start..];
    if dir_bytes.is_empty() {
        return None;
    }

    // parse directional size
    let mut dir_size = 0i32;
    let mut is_negative = false;

    if dir_bytes[0] == b'-' {
        is_negative = true;
        dir_bytes = &dir_bytes[1..];
    }

    for &b in dir_bytes {
        if b.is_ascii_digit() {
            dir_size = dir_size * 10 + i32::from(b - b'0');
        } else {
            return None;
        }
    }

    Some((
        timestamp_microsec,
        if is_negative { -dir_size } else { dir_size },
    ))
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

        let is_chaff = {
            let mut f = fs::File::open(entry.path()).map_err(ParseError::Io)?;
            let mut magic = [0u8; 5];
            if f.read_exact(&mut magic).is_ok() {
                magic == *chaff_capture::trace::TRACE_MAGIC
            } else {
                false
            }
        };

        let trace = if is_chaff {
            Trace::deserialise(&entry.path()).map_err(ParseError::ChaffSerDe)?
        } else {
            let content = fs::read_to_string(entry.path()).map_err(ParseError::Io)?;
            let mut trace_builder = TraceBuilder::default();

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

                trace_builder.record(direction, timestamp_microsec, size);
            }

            trace_builder.build()
        };

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

    // 5000 is the padding documented by the Tik-Tok authors.
    Ok(Dataset { data, pad_to: 5000 })
}
