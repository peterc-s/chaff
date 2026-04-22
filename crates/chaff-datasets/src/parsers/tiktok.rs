//! Parsing logic for [Tik-Tok](https://github.com/msrocean/Tik_Tok)
//!
//! This implements parsing from their Directional Timing (DT) representation. The dataset the authors
//! give is a collection of text files which are named as such: `<class_num>-<instance_num>`.
//!
//! Each file in DT representation consists of as many lines as packets in the trace, each in the
//! following format: `<timestamp (s)> <directional size>`.

use core::fmt;
use std::{collections::HashMap, fs, io::Read as _, path::Path};

use chaff_capture::trace::{Direction, Trace, TraceBuilder, TracePacket};

use crate::{dataset::Dataset, errors::DatasetError};

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
    let timestamp_microsec = (timestamp_sec * 1_000_000.0).round() as u64;

    // find the start of the directional size
    let mut dir_start = space_idx;
    while dir_start < bytes.len() && bytes[dir_start].is_ascii_whitespace() {
        dir_start += 1;
    }

    let mut dir_bytes = &bytes[dir_start..];

    // parse directional size
    let mut dir_size = 0i32;
    let mut is_negative = false;

    if dir_bytes[0] == b'-' {
        is_negative = true;
        dir_bytes = &dir_bytes[1..];

        if dir_bytes.is_empty() {
            return None;
        }
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

        let is_chaff = {
            let mut f = fs::File::open(entry.path()).map_err(DatasetError::Io)?;
            let mut magic = [0u8; 5];
            if f.read_exact(&mut magic).is_ok() {
                magic == *chaff_capture::trace::TRACE_MAGIC
            } else {
                false
            }
        };

        let trace = if is_chaff {
            Trace::deserialise(&entry.path()).map_err(DatasetError::ChaffSerDe)?
        } else {
            let content = fs::read_to_string(entry.path()).map_err(DatasetError::Io)?;
            let mut trace_builder = TraceBuilder::default();

            for (line_num, line) in content.lines().enumerate() {
                if line.is_empty() {
                    continue;
                }

                let (timestamp_microsec, dir_size) =
                    parse_line(line).ok_or_else(|| DatasetError::InvalidFormat {
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

/// Wrapper struct for displaying/formatting a [`Trace`] in Tik-Tok format as described
/// [here](crate::parsers::tiktok). Implements [`fmt::Display`].
pub struct TikTokDisplay<'a>(pub &'a Trace);

impl fmt::Display for TikTokDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut time = 0;
        for TracePacket(dir, delta, size) in self.0 {
            time += delta;
            let secs = time / 1_000_000;
            let micros = time % 1_000_000;
            let directional_size: i64 = match dir {
                Direction::Send => size.into(),
                Direction::Receive => -i64::from(size),
            };
            writeln!(f, "{secs}.{micros:06}\t{directional_size}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::{fs, path::PathBuf};

    use chaff_capture::trace::{Direction, TraceBuilder};
    use tempfile::tempdir;

    use crate::{
        errors::DatasetError,
        parsers::tiktok::{self, TikTokDisplay, parse_line},
    };

    #[test]
    fn test_chaff_vs_original() {
        let mut path_original = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path_original.push("test-datasets/tiktok");
        let mut path_chaff = path_original.clone();
        path_original.push("original");
        path_chaff.push("chaff");

        let original_dataset = tiktok::try_parse(path_original).unwrap();
        let chaff_dataset = tiktok::try_parse(path_chaff).unwrap();

        assert_eq!(original_dataset, chaff_dataset);
    }

    #[test]
    fn test_roundtrip_trace() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-datasets/tiktok/original/35-0");

        // parse original trace manually
        let original_str = fs::read_to_string(&path).unwrap();
        let mut builder = TraceBuilder::default();
        for line in original_str.lines() {
            if line.is_empty() {
                continue;
            }

            let (ts, dir_size) = parse_line(line).unwrap();

            let size = dir_size.unsigned_abs();
            let direction = if dir_size > 0 {
                Direction::Send
            } else {
                Direction::Receive
            };

            builder.record(direction, ts, size);
        }
        let original_trace = builder.build();

        // now format the original trace and try parse that
        let formatted = format!("{}", TikTokDisplay(&original_trace));
        let mut builder2 = TraceBuilder::default();
        for line in formatted.lines() {
            if line.is_empty() {
                continue;
            }

            let (ts, dir_size) = parse_line(line).unwrap();

            let size = dir_size.unsigned_abs();
            let direction = if dir_size > 0 {
                Direction::Send
            } else {
                Direction::Receive
            };

            builder2.record(direction, ts, size);
        }
        let reparsed_trace = builder2.build();

        // they should be equivalent
        assert_eq!(original_trace, reparsed_trace);
    }

    #[test]
    fn test_parse_line_trims_whitespace_and_tabs() {
        let (ts, dir_size) = parse_line("  1.000001\t-42  ").unwrap();
        assert_eq!(ts, 1_000_001);
        assert_eq!(dir_size, -42);
    }

    #[test]
    fn test_parse_line_rejects_missing_space_separator() {
        assert!(parse_line("1.0-42").is_none());
    }

    #[test]
    fn test_parse_line_rejects_missing_directional_size() {
        assert!(parse_line("1.23 ").is_none());
        assert!(parse_line("1.23\t").is_none());
    }

    #[test]
    fn test_parse_line_rejects_non_numeric_timestamp() {
        assert!(parse_line("abc 10").is_none());
        assert!(parse_line("1..2 10").is_none());
    }

    #[test]
    fn test_parse_line_rejects_non_numeric_directional_size() {
        assert!(parse_line("1.23 +10").is_none());
        assert!(parse_line("1.23 10x").is_none());
        assert!(parse_line("1.23 -").is_none());
        assert!(parse_line("1.23 --10").is_none());
    }

    #[test]
    fn test_try_parse_rejects_not_a_directory() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-datasets/tiktok/original/35-0");

        let err = tiktok::try_parse(&path).unwrap_err();

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
        fs::write(dir.join("1-0"), "0.000001 1\n0.000002 -1\n").unwrap();

        let dataset = tiktok::try_parse(dir).unwrap();

        let traces = dataset
            .data
            .get("1")
            .expect("expected class '1' in dataset");
        assert_eq!(traces.len(), 1);

        let t = &traces[0];
        assert_eq!(t.directions.len(), 2);
        assert_eq!(t.directions[0], Direction::Send);
        assert_eq!(t.directions[1], Direction::Receive);
    }

    #[test]
    fn test_try_parse_sorts_instances_by_instance_number() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("9-1"), "0.000001 1\n").unwrap();
        fs::write(dir.join("9-0"), "0.000001 -1\n").unwrap();

        let dataset = tiktok::try_parse(dir).unwrap();
        let traces = dataset.data.get("9").unwrap();
        assert_eq!(traces.len(), 2);

        assert_eq!(traces[0].directions[0], Direction::Receive);
        assert_eq!(traces[1].directions[0], Direction::Send);
    }

    #[test]
    fn test_parse_line_rejects_empty_after_timestamp() {
        assert!(parse_line("   1.23   ").is_none());
        assert!(parse_line("1.23 \n").is_none());
    }

    #[test]
    fn test_try_parse_skips_subdirectory_entries() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("2-0"), "0.000001 1\n").unwrap();

        let sub = dir.join("subdir");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("1-0"), "0.000001 1\n0.000002 -1\n").unwrap();

        let dataset = tiktok::try_parse(&tmp).unwrap();

        assert!(dataset.data.contains_key("2"));
        assert!(!dataset.data.contains_key("1"));

        let traces = &dataset.data["2"];
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].directions.len(), 1);
        assert_eq!(traces[0].directions[0], Direction::Send);
    }

    #[test]
    fn test_try_parse_invalid_instance_number_errors() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("1-notanumber"), "0.000001 1\n").unwrap();

        let err = tiktok::try_parse(&tmp).unwrap_err();
        match err {
            DatasetError::InvalidFileName { file, message } => {
                assert!(file.ends_with("1-notanumber"));
                assert!(message.contains("expected two unsigned integers"));
            }
            other => panic!("unexpected result: {other}"),
        }
    }

    #[test]
    fn test_try_parse_magic_read_fails_treated_as_non_chaff() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("1-0"), b"").unwrap();

        let dataset = tiktok::try_parse(&tmp).unwrap();
        let traces = dataset.data.get("1").unwrap();
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].directions.len(), 0);
    }

    #[test]
    fn test_try_parse_skips_empty_lines() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("1-0"), "0.000001 1\n\n0.000002 -1\n").unwrap();

        let dataset = tiktok::try_parse(&tmp).unwrap();
        let trace = &dataset.data["1"][0];
        assert_eq!(trace.directions.len(), 2);
        assert_eq!(trace.directions[0], Direction::Send);
        assert_eq!(trace.directions[1], Direction::Receive);
    }

    #[test]
    fn test_try_parse_invalid_line_format() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("1-0"), "0.000001 1\nthis_is_bad\n").unwrap();

        let err = tiktok::try_parse(&tmp).unwrap_err();
        match err {
            DatasetError::InvalidFormat {
                file,
                line,
                message,
            } => {
                assert!(file.ends_with("1-0"));
                assert_eq!(line, 2);
                assert!(message.contains("invalid line format"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
