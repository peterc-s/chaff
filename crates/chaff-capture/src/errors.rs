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

/// Primary error type for the `chaff_capture` crate.
#[derive(Debug)]
pub enum CaptureError {
    /// Errors relating to capture devices and packet handling.
    Device(DeviceError),

    /// Errors relating to trace serialisation and deserialisation.
    Trace(TraceError),

    /// Conversion between types is not possible.
    CantConvert,
}

impl Error for CaptureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Device(e) => Some(e),
            Self::Trace(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Device(e) => write!(f, "device error: {e}"),
            Self::Trace(e) => write!(f, "trace error: {e}"),
            Self::CantConvert => write!(f, "conversion between types failed"),
        }
    }
}

impl From<pcap::Error> for CaptureError {
    fn from(err: pcap::Error) -> Self {
        Self::Device(DeviceError::Pcap(err))
    }
}

impl_from!(DeviceError, CaptureError, CaptureError::Device);
impl_from!(TraceError, CaptureError, CaptureError::Trace);

/// Errors relating to capture devices, packet handling, and MAC addresses.
#[derive(Debug)]
pub enum DeviceError {
    /// When no suitable capture device is found.
    NoDevice,

    /// When the capture thread fails in some way.
    CaptureThreadPanic,

    /// A packet was found to be invalid while checking for packet directions.
    InvalidPacket(String),

    /// Wrapped errors from the [`pcap`] crate.
    Pcap(pcap::Error),

    /// Wrapped errors from the [`mac_address`] crate.
    MacAddress(mac_address::MacAddressError),

    /// Couldn't get a MAC address for a device.
    NoMac(String),
}

impl Error for DeviceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pcap(e) => Some(e),
            Self::MacAddress(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no capture device found."),
            Self::CaptureThreadPanic => write!(f, "capture thread panicked."),
            Self::InvalidPacket(msg) => write!(f, "invalid packet: {msg}"),
            Self::Pcap(inner) => write!(f, "pcap error: {inner}"),
            Self::MacAddress(inner) => write!(f, "mac address lookup failed: {inner}"),
            Self::NoMac(device) => write!(f, "could not get mac address for device: {device}"),
        }
    }
}

impl_from!(pcap::Error, DeviceError, Self::Pcap);
impl_from!(mac_address::MacAddressError, DeviceError, Self::MacAddress);

/// Errors relating to trace file serialisation and deserialisation.
#[derive(Debug)]
pub enum TraceError {
    /// A [`crate::trace::Trace`]'s fields have mismatched lengths.
    LengthMismatch(usize, usize, usize),

    /// An I/O error during serialisation or deserialisation.
    Io(io::Error),

    /// Deserialised trace file has invalid magic bytes.
    InvalidMagic(Box<[u8]>),

    /// Deserialised trace file has an invalid version.
    InvalidVersion(Box<[u8]>),

    /// Trace file ended unexpectedly during deserialisation.
    UnexpectedEof,
}

impl Error for TraceError {}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch(directions, timing_deltas, sizes) => write!(
                f,
                "mismatched trace field lengths — directions: {directions}, \
                 timing_deltas: {timing_deltas}, sizes: {sizes}."
            ),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::InvalidMagic(magic) => write!(f, "invalid trace file magic bytes: {magic:?}"),
            Self::InvalidVersion(version) => write!(f, "invalid trace file version: {version:?}"),
            Self::UnexpectedEof => write!(f, "trace file ended unexpectedly."),
        }
    }
}

impl_from!(io::Error, TraceError, Self::Io);
