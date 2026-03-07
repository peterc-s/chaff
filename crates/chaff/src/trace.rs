//! The different representations of traces and methods for working with traces.

#![allow(dead_code)]
// TODO: Serialising to some standard format(s)?

use std::{error::Error, fs::OpenOptions, io::Write as _, path::Path};

use crate::errors::TraceError;

/// Packet direction.
#[derive(Debug)]
pub enum Direction {
    /// From client to server.
    Send,

    /// From server to client.
    Receive,
}

/// Represents a trace explicitly with packet [Direction]s, packet timing deltas, and sizes
/// (assumes non-fixed transmission unit size). Fixed-size.
#[derive(Debug)]
pub struct Trace {
    /// The direction in which a packet was send.
    pub directions: Box<[Direction]>,

    /// How long between last packet and this one.
    pub timing_deltas: Box<[u64]>,

    /// Assuming largest MTU is 4GiB (IPv6 jumbograms, for example).
    pub sizes: Box<[u32]>,
}

const TRACE_MAGIC: &[u8; 5] = b"CHAFF";
const TRACE_VERSION: &[u8; 3] = &[0, 1, 0];

impl Trace {
    fn len_check(&self) -> Result<(), TraceError> {
        let directions_len = self.directions.len();
        let timing_deltas_len = self.timing_deltas.len();
        let sizes_len = self.sizes.len();

        if directions_len != timing_deltas_len || timing_deltas_len != sizes_len {
            Err(TraceError::LengthMismatch(
                directions_len,
                timing_deltas_len,
                sizes_len,
            ))
        } else {
            Ok(())
        }
    }

    fn serialise(&self, to: &Path) -> Result<(), Box<dyn Error>> {
        // Length-sensitive operation, should check field lengths are the same
        // before.
        self.len_check()?;

        let trace_len = self.directions.len();
        let mut buf: Vec<u8> = vec![];

        // header:
        // - bytes 0-4: magic bytes
        // - bytes 5-7: version
        // - bytes 8-16: trace length
        buf.extend(TRACE_MAGIC);
        buf.extend(TRACE_VERSION);
        buf.extend(&(trace_len as u64).to_le_bytes());

        buf.extend(self.directions.iter().map(|d| match d {
            Direction::Send => 0u8,
            Direction::Receive => 1u8,
        }));

        for delta in &self.timing_deltas {
            buf.extend_from_slice(&delta.to_le_bytes());
        }

        for size in &self.sizes {
            buf.extend_from_slice(&size.to_le_bytes());
        }

        let mut out = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(to)?;

        out.write_all(&buf)?;

        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn test_serialise_trace() {
        // create dummy trace
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([10, 20, 30]),
            sizes: Box::new([100, 200, 300]),
        };

        // save to a temporary file
        let path = temp_file("test_trace.bin");

        // serialise
        trace.serialise(&path).unwrap();

        // read file back
        let bytes = fs::read(&path).unwrap();

        // check header
        assert_eq!(&bytes[0..5], TRACE_MAGIC);
        assert_eq!(&bytes[5..8], TRACE_VERSION);
        assert_eq!(u64::from_le_bytes(bytes[8..16].try_into().unwrap()), 3);

        // check data
        // directions
        assert_eq!(&bytes[16..19], &[0, 1, 0]);

        // timing deltas
        let delta_0 = u64::from_le_bytes(bytes[19..27].try_into().unwrap());
        let delta_1 = u64::from_le_bytes(bytes[27..35].try_into().unwrap());
        let delta_2 = u64::from_le_bytes(bytes[35..43].try_into().unwrap());
        assert_eq!(delta_0, trace.timing_deltas[0]);
        assert_eq!(delta_1, trace.timing_deltas[1]);
        assert_eq!(delta_2, trace.timing_deltas[2]);

        // sizes
        let size_0 = u32::from_le_bytes(bytes[43..47].try_into().unwrap());
        let size_1 = u32::from_le_bytes(bytes[47..51].try_into().unwrap());
        let size_2 = u32::from_le_bytes(bytes[51..55].try_into().unwrap());
        assert_eq!(size_0, trace.sizes[0]);
        assert_eq!(size_1, trace.sizes[1]);
        assert_eq!(size_2, trace.sizes[2]);
    }
}
