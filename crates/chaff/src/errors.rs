//! Standard error definitions and [`std::fmt::Display`] immplementations.

use std::{error::Error, fmt};

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
}

impl Error for ChaffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Capture(e) => Some(e),
        }
    }
}

impl fmt::Display for ChaffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture(e) => write!(f, "capture error: {e}"),
        }
    }
}

impl_from!(CaptureError, ChaffError, ChaffError::Capture);

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
