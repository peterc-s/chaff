//! The different representations of traces and methods for working with traces.

use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Read as _, Write as _},
    path::Path,
};

use crate::errors::{ChaffError, TraceError};

/// Packet direction.
#[derive(Debug, PartialEq, Eq)]
pub enum Direction {
    /// From client to server.
    Send,

    /// From server to client.
    Receive,
}

/// Represents a trace explicitly with packet [Direction]s, packet timing deltas, and sizes
/// (assumes non-fixed transmission unit size). Fixed-size, field lengths should match.
#[derive(Debug, PartialEq, Eq)]
pub struct Trace {
    /// The direction in which a packet was send.
    pub directions: Box<[Direction]>,

    /// How long between last packet and this one.
    pub timing_deltas: Box<[u32]>,

    /// Assuming largest MTU is 4GiB (IPv6 jumbograms, for example).
    pub sizes: Box<[u32]>,
}

/// Trace binary header information.
const TRACE_MAGIC: &[u8; 5] = b"CHAFF";
const TRACE_VERSION: &[u8; 3] = &[0, 1, 0];

impl Trace {
    /// Should be used before a size-sensitive operation. Errors if lengths mismatch.
    fn len_check(&self) -> Result<(), ChaffError> {
        let directions_len = self.directions.len();
        let timing_deltas_len = self.timing_deltas.len();
        let sizes_len = self.sizes.len();

        if directions_len != timing_deltas_len || timing_deltas_len != sizes_len {
            Err(TraceError::LengthMismatch(directions_len, timing_deltas_len, sizes_len).into())
        } else {
            Ok(())
        }
    }

    /// Serialise a [`Trace`] to a binary-format file.
    ///
    /// Header:
    /// - bytes 0-4: magic bytes ("CHAFF")
    /// - bytes 5-7: version (semantic versioning triple)
    /// - bytes 8-12: trace length (u32)
    ///
    /// Then, the [`Trace`] fields are written one after the other in the following order:
    /// - [`Trace::directions`]: 0 for [`Direction::Send`], 1 for [`Direction::Receive`]
    /// - [`Trace::timing_deltas`]
    /// - [`Trace::sizes`]
    pub fn serialise<P: AsRef<Path>>(&self, to: &P) -> Result<(), ChaffError> {
        // Length-sensitive operation, should check field lengths are the same
        // before.
        self.len_check()?;

        let trace_len = self.directions.len();
        let mut buf: Vec<u8> = vec![];

        // header:
        // - bytes 0-4: magic bytes
        // - bytes 5-7: version
        // - bytes 8-12: trace length
        buf.extend(TRACE_MAGIC);
        buf.extend(TRACE_VERSION);

        // we truncate to 32-bit for compatibility
        #[expect(clippy::cast_possible_truncation)]
        buf.extend(&(trace_len as u32).to_le_bytes());

        // tarpaulin can't seem to figure out this is covered by the round trip test.
        #[cfg(not(tarpaulin_include))]
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
            .open(to)
            .map_err(TraceError::Io)?;

        out.write_all(&buf).map_err(TraceError::Io)?;

        Ok(())
    }

    /// Deserialise a trace from a binary-format file, see [`Trace::serialise()`] for more
    /// information on the format.
    pub fn deserialise<P: AsRef<Path>>(path: &P) -> Result<Self, ChaffError> {
        // read file
        let bytes = fs::read(path).map_err(TraceError::Io)?;
        let mut cursor = Cursor::new(&bytes);
        let mut read = |buf: &mut [u8]| -> Result<(), TraceError> {
            cursor
                .read_exact(buf)
                .map_err(|_| TraceError::UnexpectedEof)
        };

        // validate header
        let mut magic = [0u8; 5];
        read(&mut magic)?;
        if magic != *TRACE_MAGIC {
            return Err(TraceError::InvalidMagic(magic.into()).into());
        }

        let mut version = [0u8; 3];
        read(&mut version)?;
        if version != *TRACE_VERSION {
            return Err(TraceError::InvalidVersion(version.into()).into());
        }

        // read length
        let mut len_buf = [0u8; 4];
        read(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;

        // read directions
        let mut dir_buf = vec![0u8; len];
        read(&mut dir_buf)?;
        let directions = dir_buf
            .iter()
            .map(|&b| match b {
                0 => Ok(Direction::Send),
                1 => Ok(Direction::Receive),
                n => Err(TraceError::InvalidDirection(n)),
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        // Read timing deltas
        let timing_deltas = (0..len)
            .map(|_| {
                let mut b = [0u8; 4];
                read(&mut b).map(|()| u32::from_le_bytes(b))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        // Read sizes
        let sizes = (0..len)
            .map(|_| {
                let mut b = [0u8; 4];
                read(&mut b).map(|()| u32::from_le_bytes(b))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        // tarpaulin can't seem to figure out this is covered by the round trip test.
        #[cfg(not(tarpaulin_include))]
        Ok(Self {
            directions,
            timing_deltas,
            sizes,
        })
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
        let path = temp_file("test_serialise_trace.bin");
        trace.serialise(&path).unwrap();

        // read file back
        let bytes = fs::read(&path).unwrap();

        // check header
        assert_eq!(&bytes[0..5], TRACE_MAGIC);
        assert_eq!(&bytes[5..8], TRACE_VERSION);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 3);

        // check data
        // directions
        assert_eq!(&bytes[12..15], &[0, 1, 0]);

        let mut off = 15;
        let inc = size_of::<u32>();

        // timing deltas
        for delta in &trace.timing_deltas {
            let read_delta = u32::from_le_bytes(bytes[off..off + inc].try_into().unwrap());
            off += inc;
            assert_eq!(read_delta, *delta);
        }

        // sizes
        for size in &trace.sizes {
            let read_size = u32::from_le_bytes(bytes[off..off + inc].try_into().unwrap());
            off += inc;
            assert_eq!(read_size, *size);
        }

        // check total size
        assert_eq!(bytes.len(), off);
    }

    #[test]
    fn test_serialise_length_mismatch() {
        // create dummy trace
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([10, 20, 30]),
            sizes: Box::new([100, 200]),
        };

        let path = temp_file("test_serialise_length_mismatch.bin");

        let result = trace.serialise(&path);

        // check for specific error
        match result {
            Err(ChaffError::Trace(TraceError::LengthMismatch(3, 3, 2))) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_deserialise_invalid_direction() {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(TRACE_MAGIC);
        bytes.extend_from_slice(TRACE_VERSION);
        bytes.extend_from_slice(&(1u32.to_le_bytes()));

        bytes.push(2); // invalid direction

        bytes.extend_from_slice(&0u32.to_le_bytes()); // timing delta
        bytes.extend_from_slice(&0u32.to_le_bytes()); // size

        let file = temp_file("test_deserialise_invalid_direction.bin");
        fs::write(&file, bytes).unwrap();

        let result = Trace::deserialise(&file);

        // check for specific error
        match result {
            Err(ChaffError::Trace(TraceError::InvalidDirection(2))) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_deserialise_invalid_magic() {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(b"WRONG");
        bytes.extend_from_slice(TRACE_VERSION);
        bytes.extend_from_slice(&(1u32.to_le_bytes())); // len = 1

        bytes.push(0u8);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // timing delta
        bytes.extend_from_slice(&0u32.to_le_bytes()); // size

        let file = temp_file("test_deserialise_invalid_magic.bin");
        fs::write(&file, bytes).unwrap();

        let result = Trace::deserialise(&file);

        // check for specific error
        match result {
            Err(ChaffError::Trace(TraceError::InvalidMagic(found))) => {
                let found: [u8; 5] = found[0..5].try_into().unwrap();
                assert_eq!(&found, b"WRONG");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_deserialise_invalid_version() {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(TRACE_MAGIC);
        bytes.extend_from_slice(&[255u8, 255u8, 255u8]);
        bytes.extend_from_slice(&(1u32.to_le_bytes())); // len = 1

        bytes.push(0u8);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // timing delta
        bytes.extend_from_slice(&0u32.to_le_bytes()); // size

        let file = temp_file("test_deserialise_invalid_magic.bin");
        fs::write(&file, bytes).unwrap();

        let result = Trace::deserialise(&file);

        // check for specific error
        match result {
            Err(ChaffError::Trace(TraceError::InvalidVersion(found))) => {
                assert_eq!(*found, [255u8, 255u8, 255u8]);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_serde_round_trip() {
        // create dummy trace
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([10, 20, 30]),
            sizes: Box::new([100, 200, 300]),
        };

        // save to a temporary file
        let path = temp_file("test_serde_round_trip.bin");
        trace.serialise(&path).unwrap();

        // check trace is the same
        let trace_deserialised = Trace::deserialise(&path).unwrap();
        assert_eq!(trace, trace_deserialised);
    }
}
