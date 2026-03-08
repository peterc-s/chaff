//! Standard error definitions and [`std::fmt::Display`] implementations.

// These aren't testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt, io};

macro_rules! impl_from {
    ($from:ty, $to:ty, $variant:expr) => {
        impl From<$from> for $to {
            fn from(err: $from) -> Self {
                $variant(err)
            }
        }
    };
}

/// Primary error type for Chaff
#[derive(Debug)]
pub enum ChaffError {
    /// Errors from [`crate::capture`].
    Capture(CaptureError),

    /// Errors from [`crate::trace`]
    Trace(TraceError),
}

impl Error for ChaffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Capture(e) => Some(e),
            Self::Trace(e) => Some(e),
        }
    }
}

impl fmt::Display for ChaffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture(e) => write!(f, "capture error: {e}"),
            Self::Trace(e) => write!(f, "trace error: {e}"),
        }
    }
}

impl_from!(CaptureError, ChaffError, ChaffError::Capture);
impl_from!(TraceError, ChaffError, ChaffError::Trace);

/// Capture error type for the [`crate::capture`] module.
#[derive(Debug)]
pub enum CaptureError {
    /// When no suitable device is found.
    NoDevice,

    /// When the capture thread fails in some way.
    CaptureThreadPanic,

    /// A packet received or sent was found to be invalid while checking for packet directions.
    InvalidPacket(String),

    /// Wrapped errors from the [`pcap`] crate.
    Pcap(pcap::Error),

    /// Wrapped errors from the [`mac_address`] crate.
    MacAddress(mac_address::MacAddressError),

    /// Couldn't get MAC address for a device.
    NoMac(String),
}

impl Error for CaptureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pcap(e) => Some(e),
            Self::MacAddress(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no capture device found."),
            Self::CaptureThreadPanic => write!(f, "capture thread panicked."),
            Self::InvalidPacket(msg) => write!(f, "invalid packet found: {msg}"),
            Self::Pcap(inner) => write!(f, "pcap error: {inner}"),
            Self::MacAddress(inner) => write!(f, "mac address lookup failed: {inner}"),
            Self::NoMac(device) => write!(f, "could not get mac address for device: {device}"),
        }
    }
}

impl_from!(pcap::Error, CaptureError, Self::Pcap);
impl_from!(mac_address::MacAddressError, CaptureError, Self::MacAddress);

/// Trace error type for the [`crate::trace`] module.
#[derive(Debug)]
pub enum TraceError {
    /// When a [`crate::trace::Trace`]'s fields have mis-matched lengths.
    LengthMismatch(usize, usize, usize),

    /// An I/O error when serialising or deserialising.
    Io(io::Error),

    /// Deserialised trace file has invalid magic bytes.
    InvalidMagic(Box<[u8]>),

    /// Deserialised trace file has invalid version.
    InvalidVersion(Box<[u8]>),

    /// Deserialised trace file ended unexpectedly.
    UnexpectedEof,
}

impl Error for TraceError {}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch(directions, timing_deltas, sizes) => write!(
                f,
                "mis-matched trace field lengths. directions: {directions}, timing_deltas: {timing_deltas}, sizes: {sizes}."
            ),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::InvalidMagic(magic) => {
                write!(f, "invalid trace file magic bytes: {magic:?}")
            }
            Self::InvalidVersion(version) => {
                write!(f, "invalid trace file version: {version:?}")
            }
            Self::UnexpectedEof => write!(f, "trace file ended unexpectedly."),
        }
    }
}

impl_from!(io::Error, TraceError, Self::Io);
