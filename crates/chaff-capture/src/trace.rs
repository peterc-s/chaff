//! The different representations of traces and methods for working with traces.

use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Read as _, Write as _},
    path::Path,
};

use chaff::event::Event;

use crate::errors::{CaptureError, TraceError};

/// Packet direction.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Direction {
    /// From client to server.
    Send,

    /// From server to client.
    Receive,
}

impl TryFrom<Event> for Direction {
    type Error = CaptureError;

    fn try_from(value: Event) -> Result<Self, Self::Error> {
        match value {
            Event::SendNormal | Event::SendDecoy => Ok(Self::Send),
            Event::ReceiveNormal | Event::ReceiveDecoy => Ok(Self::Receive),
            Event::QueuePopped(_) | Event::QueueFull(_) | Event::StateBudgetExhausted => {
                Err(CaptureError::CantConvert)
            }
        }
    }
}

/// A builder for [`Trace`]. Allows building up a [`Trace`] by repeatedly adding packets to the end
/// with [`TraceBuilder::record`] and then building with [`TraceBuilder::build`].
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct TraceBuilder {
    directions: Vec<Direction>,
    timing_deltas: Vec<u32>,
    sizes: Vec<u32>,
    last_ts: u64,
}

impl TraceBuilder {
    /// Create a new [`TraceBuilder`] with the given initial timestamp.
    #[must_use]
    pub fn new(initial_ts: u64) -> Self {
        Self {
            directions: vec![],
            timing_deltas: vec![],
            sizes: vec![],
            last_ts: initial_ts,
        }
    }

    /// Add a packet to the [`TraceBuilder`].
    pub fn record(&mut self, dir: Direction, time: u64, size: u32) {
        self.directions.push(dir);
        #[expect(clippy::cast_possible_truncation)]
        self.timing_deltas.push((time - self.last_ts) as u32);
        self.sizes.push(size);
        self.last_ts = time;
    }

    /// Build a [`Trace`] out of this [`TraceBuilder`].
    #[must_use]
    pub fn build(self) -> Trace {
        Trace {
            directions: self.directions.into_boxed_slice(),
            timing_deltas: self.timing_deltas.into_boxed_slice(),
            sizes: self.sizes.into_boxed_slice(),
        }
    }
}

/// Represents a trace explicitly with packet [Direction]s, packet timing deltas, and sizes
/// (assumes non-fixed transmission unit size). Fixed-size, field lengths should match.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// The direction in which a packet was send.
    pub directions: Box<[Direction]>,

    /// How long between last packet and this one.
    pub timing_deltas: Box<[u32]>,

    /// Assuming largest MTU is 4GiB (IPv6 jumbograms, for example).
    pub sizes: Box<[u32]>,
}

/// Trace binary header information.
pub const TRACE_MAGIC: &[u8; 5] = b"CHAFF";

/// Current version of the Chaff trace format.
pub const TRACE_VERSION: &[u8; 3] = &[0, 1, 0];

impl Trace {
    /// Should be used before a size-sensitive operation. Errors if lengths mismatch.
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

    /// Serialise a [`Trace`] to a binary-format file.
    ///
    /// Header:
    /// - bytes 0-4: magic bytes ("CHAFF")
    /// - bytes 5-7: version (semantic versioning triple)
    /// - bytes 8-12: trace length (u32)
    ///
    /// Then, the [`Trace`] fields are written one after the other in the following order:
    /// - [`Trace::directions`]: 0 for [`Direction::Send`], 1 for [`Direction::Receive`], packed
    ///   LSB-first into a bitvector so packet `i` occupies bit `i % 8` of byte `i / 8`.
    /// - [`Trace::timing_deltas`].
    /// - [`Trace::sizes`].
    ///
    /// All fields are encoded little-endian.
    ///
    /// # Errors
    ///
    /// Can error if the [`Trace`]'s internal vector lengths mismatch, or if there is some error
    /// when opening or writing to the given path.
    pub fn serialise<P: AsRef<Path>>(&self, to: &P) -> Result<(), TraceError> {
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

        // pack directions into a bitvector
        let mut packed_dirs = vec![0u8; trace_len.div_ceil(8)];
        for (i, dir) in self.directions.iter().enumerate() {
            if matches!(dir, Direction::Receive) {
                packed_dirs[i / 8] |= 1 << (i % 8);
            }
        }
        buf.extend(packed_dirs);

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
    ///
    /// # Errors
    ///
    /// May error if there are errors when reading the file, if the file's magic bytes don't match
    /// the expected magic bytes ("CHAFF"), if the version is incompatible, or if the resulting
    /// [`Trace`]'s internal vector lengths mismatch.
    pub fn deserialise<P: AsRef<Path>>(path: &P) -> Result<Self, TraceError> {
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
            return Err(TraceError::InvalidMagic(magic.into()));
        }

        let mut version = [0u8; 3];
        read(&mut version)?;
        if version != *TRACE_VERSION {
            return Err(TraceError::InvalidVersion(version.into()));
        }

        // read length
        let mut len_buf = [0u8; 4];
        read(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;

        // read directions
        let mut dir_buf = vec![0u8; len.div_ceil(8)];
        read(&mut dir_buf)?;

        // can't test unreachable...
        #[cfg(not(tarpaulin_include))]
        let directions = (0..len)
            .map(|i| {
                let bit = (dir_buf[i / 8] >> (i % 8)) & 1;
                match bit {
                    0 => Direction::Send,
                    1 => Direction::Receive,
                    _ => unreachable!(),
                }
            })
            .collect::<Vec<Direction>>()
            .into_boxed_slice();

        // read timing deltas
        let timing_deltas = (0..len)
            .map(|_| {
                let mut b = [0u8; 4];
                read(&mut b).map(|()| u32::from_le_bytes(b))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        // read sizes
        let sizes = (0..len)
            .map(|_| {
                let mut b = [0u8; 4];
                read(&mut b).map(|()| u32::from_le_bytes(b))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        let result = Self {
            directions,
            timing_deltas,
            sizes,
        };

        // an io::Error _should_ have happened if lengths were wrong, this ensures lengths are
        // correct.
        result.len_check()?;

        // tarpaulin can't seem to figure out this is covered by the round trip test.
        #[cfg(not(tarpaulin_include))]
        Ok(result)
    }

    /// Returns a [`TraceIter`], an iterator over a trace, which gives ([`Direction`], [`u32`],
    /// [`u32`]) items as [`Trace`] is a Struct-of-Arrays.
    #[must_use]
    pub fn iter(&self) -> TraceIter<'_> {
        self.into_iter()
    }

    /// Returns the length of the trace. Assumes all internal arrays are of equal length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.directions.len()
    }

    /// Checks if the trace has any packets. Assumes all internal arrays are of equal length.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.directions.is_empty()
    }
}

impl std::fmt::Display for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for TracePacket(dir, delta, size) in self {
            writeln!(f, "+{delta} {dir:?}: {size}")?;
        }
        Ok(())
    }
}

/// An iterator over a [`Trace`].
pub struct TraceIter<'a> {
    trace: &'a Trace,
    index: usize,
}

/// A single packet in a trace.
pub struct TracePacket(pub Direction, pub u32, pub u32);

impl Iterator for TraceIter<'_> {
    type Item = TracePacket;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.trace.directions.len() {
            return None;
        }

        let i = self.index;
        self.index += 1;

        Some(TracePacket(
            self.trace.directions[i],
            self.trace.timing_deltas[i],
            self.trace.sizes[i],
        ))
    }
}

impl<'a> IntoIterator for &'a Trace {
    type Item = TracePacket;

    type IntoIter = TraceIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        TraceIter {
            trace: self,
            index: 0,
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::expect_used)]
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
        assert_eq!(&bytes[12], &0b0000_0010u8);

        let mut off = 13;
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
            Err(TraceError::LengthMismatch(3, 3, 2)) => {}
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
            Err(TraceError::InvalidMagic(found)) => {
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

        let file = temp_file("test_deserialise_invalid_version.bin");
        fs::write(&file, bytes).unwrap();

        let result = Trace::deserialise(&file);

        // check for specific error
        match result {
            Err(TraceError::InvalidVersion(found)) => {
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

    #[test]
    fn test_trace_is_empty() {
        let empty_trace = Trace {
            directions: Box::new([]),
            timing_deltas: Box::new([]),
            sizes: Box::new([]),
        };
        assert!(empty_trace.is_empty());
        assert_eq!(empty_trace.len(), 0);

        let non_empty_trace = Trace {
            directions: Box::new([Direction::Send]),
            timing_deltas: Box::new([0]),
            sizes: Box::new([0]),
        };
        assert!(!non_empty_trace.is_empty());
        assert_eq!(non_empty_trace.len(), 1);
    }

    #[test]
    fn test_trace_iterator() {
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive]),
            timing_deltas: Box::new([10, 20]),
            sizes: Box::new([100, 200]),
        };

        let mut iter = trace.iter();

        let p1 = iter.next().expect("Should have first packet");
        assert!(matches!(p1.0, Direction::Send));
        assert_eq!(p1.1, 10);
        assert_eq!(p1.2, 100);

        let p2 = iter.next().expect("Should have second packet");
        assert!(matches!(p2.0, Direction::Receive));
        assert_eq!(p2.1, 20);
        assert_eq!(p2.2, 200);

        assert!(iter.next().is_none());

        let collected = (&trace).into_iter();
        assert_eq!(collected.count(), 2);
    }

    #[test]
    fn test_trace_display_format() {
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive]),
            timing_deltas: Box::new([5, 15]),
            sizes: Box::new([64, 1500]),
        };

        let output = format!("{trace}");
        let expected = "+5 Send: 64\n+15 Receive: 1500\n";

        assert_eq!(output, expected);
    }

    #[test]
    fn test_try_from_queue_cant_convert() {
        assert!(matches!(
            Direction::try_from(Event::QueuePopped(0)).unwrap_err(),
            CaptureError::CantConvert
        ));
    }
}
