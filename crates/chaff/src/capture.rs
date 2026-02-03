//! Use [libpcap](https://github.com/the-tcpdump-group/libpcap) through the [pcap] crate to capture
//! a [`crate::trace::Trace`].

use std::{error::Error, fmt, time::Duration};

use mac_address::mac_address_by_name;
use pcap::{Capture, Device, Inactive, Linktype, PacketHeader};

use crate::trace::{Direction, Trace};

/// Capture error type.
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

impl Error for CaptureError {}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "No capture device found."),
            Self::CaptureThreadPanic => write!(f, "Capture thread panicked."),
            Self::InvalidPacket(msg) => write!(f, "Invalid packet found: {msg}"),
            Self::Pcap(inner) => write!(f, "Error from pcap: {inner}"),
            Self::MacAddress(inner) => write!(f, "Error from mac_address: {inner}"),
            Self::NoMac(device) => write!(f, "Couldn't get MAC address for device: {device}"),
        }
    }
}

impl From<pcap::Error> for CaptureError {
    fn from(err: pcap::Error) -> Self {
        Self::Pcap(err)
    }
}

impl From<mac_address::MacAddressError> for CaptureError {
    fn from(err: mac_address::MacAddressError) -> Self {
        Self::MacAddress(err)
    }
}

/// Creates an [`pcap::Capture<Inactive>`]. Selects a device via [`pcap::Device::lookup()`].
pub fn create_capture() -> Result<Capture<Inactive>, CaptureError> {
    let device = Device::lookup()?;

    // Create a capture from the device if one exists.
    if let Some(device) = device {
        Ok(Capture::from_device(device)?)
    } else {
        Err(CaptureError::NoDevice)
    }
}

/// Activates the given `capture` for `ms` milliseconds and produces a [`crate::trace::Trace`].
#[expect(clippy::dbg_macro)]
pub fn capture_for_ms(duration: Duration) -> Result<Trace, CaptureError> {
    // TODO: move this out if possible, but need device for direction
    let device = Device::lookup()?.ok_or(CaptureError::NoDevice)?;

    dbg!(&device);

    let capture = Capture::from_device(device.clone())?;

    let mut open_cap = capture.open()?;
    let linktype = open_cap.get_datalink();

    let break_handle = open_cap.breakloop_handle();

    let capture_thread = std::thread::spawn(move || {
        let mut packets: Vec<(PacketHeader, Vec<u8>)> = Vec::new();
        loop {
            let maybe_pkt = open_cap.next_packet();
            if let Ok(pkt) = maybe_pkt {
                // FIXME: this is probably slow, profile and if it is, use one big vec with
                // offsets or don't store the data at all (other than bytes useful for packet direction?)
                packets.push((*pkt.header, pkt.data.to_vec()));
            } else if matches!(maybe_pkt, Err(pcap::Error::NoMorePackets)) {
                println!("Timeout expired.");
                break;
            } else if let Err(e) = maybe_pkt {
                return Err(e);
            }
        }
        Ok(packets)
    });

    std::thread::spawn(move || {
        std::thread::sleep(duration);
        break_handle.breakloop();
    });

    let packets = capture_thread
        .join()
        .map_err(|_| CaptureError::CaptureThreadPanic)??;

    packets_to_trace(&packets, linktype, device)
}

/// Converts packet information into traces
// TODO: Abstract this out a bit.
// TODO: add error handling.
fn packets_to_trace(
    packets: &[(PacketHeader, Vec<u8>)],
    linktype: Linktype,
    device: Device,
) -> Result<Trace, CaptureError> {
    // TODO: maybe move this elsewhere
    #[expect(clippy::cast_sign_loss)]
    fn packet_ts_to_ms(header: PacketHeader) -> u64 {
        let tv = header.ts;
        let sec_ms = (tv.tv_sec as u64) * 1000;
        let usec_ms = (tv.tv_usec as u64) / 1000;

        sec_ms + usec_ms
    }

    let mac_address = mac_address_by_name(&device.name)?
        .ok_or(CaptureError::NoMac(device.name))?
        .bytes();

    // Get directions vector
    let directions = match linktype {
        // Reference: https://ieeexplore.ieee.org/document/7428776
        Linktype::ETHERNET => packets
            .iter()
            .map(|(_, data)| {
                // if data.len() < 14 {
                //     todo!("This should cause an error, packet couldn't possibly be this size.")
                // }

                // header has 6 octets for dest, then 6 bytes for src
                let src_mac_address = &data[6..12];

                if mac_address == src_mac_address {
                    Direction::Send
                } else {
                    Direction::Receive
                }
            })
            .collect(),

        // Reference: https://www.tcpdump.org/linktypes/LINKTYPE_LINUX_SLL.html
        Linktype::LINUX_SLL => packets
            .iter()
            .map(|(_, data)| {
                // if data.len() < 16 {
                //     todo!("This should cause an error as the packet header is 16 bytes minimum.")
                // }

                // first two octets is the packet type
                let header_packet_type = &data[0..2];

                // big-endian convert first two bytes to u16
                let packet_type =
                    (u16::from(header_packet_type[0]) << 8) | u16::from(header_packet_type[1]);

                match packet_type {
                    // 0 - unicast to us
                    // 1 - broadcast by someone else
                    // 2 - multicast, not broadcast, by someone else
                    // 3 - unicast by someone else to someone else
                    0_u16..=3_u16 => Direction::Receive,
                    // 4 - sent by us
                    // 4_u16 => Direction::Send,
                    _ => Direction::Send, // TODO: This should probably cause an error as this is an invalid type.
                }
            })
            .collect(),

        // Reference: https://www.tcpdump.org/linktypes/LINKTYPE_LINUX_SLL2.html
        Linktype::LINUX_SLL2 => packets
            .iter()
            .map(|(_, data)| {
                // if data.len() < 20 {
                //     todo!("This should cause an error as the packet header is 20 bytes minimum.")
                // }

                // packet type is at 10th index
                match &data[10] {
                    // 0 - unicast to us
                    // 1 - broadcast by someone else
                    // 2 - multicast, not broadcast, by someone else
                    // 3 - unicast by someone else to someone else
                    0_u8..=3_u8 => Direction::Receive,
                    // 4 - sent by us
                    // 4_u8 => Direction::Send,
                    _ => Direction::Send, // TODO: This should probably cause an error as this is an invalid type.
                }
            })
            .collect(),

        _ => unimplemented!(),
    };

    let timing_deltas: Box<[u64]> = std::iter::once(0)
        .chain(
            packets
                .windows(2)
                .map(|w| packet_ts_to_ms(w[1].0).saturating_sub(packet_ts_to_ms(w[0].0))),
        )
        .collect();

    let sizes: Box<[u32]> = packets.iter().map(|pkt| pkt.0.len).collect();

    Ok(Trace {
        directions,
        timing_deltas,
        sizes,
    })
}
