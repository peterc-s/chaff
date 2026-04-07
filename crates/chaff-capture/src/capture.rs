//! Use [libpcap](https://github.com/the-tcpdump-group/libpcap) through the [pcap] crate to capture
//! a [`crate::trace::Trace`].

use std::time::Duration;

use mac_address::{MacAddress, mac_address_by_name};
use pcap::{Capture, Device, Linktype, PacketHeader};

use crate::errors::{DeviceError, TraceError};
use crate::trace::TraceBuilder;
use crate::{
    errors::CaptureError,
    trace::{Direction, Trace},
};

/// Find an interface with the given `ifname`.
// This isn't unit testable because it requires knowledge about the machine the test is running on.
#[cfg(not(tarpaulin_include))]
pub fn find_interface(ifname: &String) -> Result<Option<Device>, pcap::Error> {
    Ok(Device::list()?.into_iter().find(|dev| dev.name == *ifname))
}

/// Get the [`MacAddress`] of a [`Device`].
// This isn't unit testable because it requires either spoofing or knowing the device name and MAC
// address of said device.
#[cfg(not(tarpaulin_include))]
fn get_device_mac(device: &Device) -> Result<MacAddress, DeviceError> {
    mac_address_by_name(&device.name)?.ok_or_else(|| DeviceError::NoMac(device.name.clone()))
}

/// Activates the given `capture` for the given [`Duration`] and produces a [`crate::trace::Trace`].
///
/// Optionally pass in a device. If no device given, look up a device with [`pcap::Device::lookup()`].
// This isn't unit testable because libpcap requires specific capabilities (that would
// require running as root or setting capabilities which requires root).
#[cfg(not(tarpaulin_include))]
pub fn capture_for(duration: Duration, device: Option<Device>) -> Result<Trace, CaptureError> {
    let device = device.unwrap_or(Device::lookup()?.ok_or(DeviceError::NoDevice)?);
    let mac_address = get_device_mac(&device)?;

    let capture = Capture::from_device(device)?;
    let mut open_cap = capture.open()?;
    let linktype = open_cap.get_datalink();

    let break_handle = open_cap.breakloop_handle();
    let capture_thread = std::thread::spawn(move || {
        // TODO: tune capacity to minimise reallocs?
        let mut packets: Vec<(PacketHeader, Vec<u8>)> = Vec::with_capacity(4096);
        loop {
            let maybe_pkt = open_cap.next_packet();
            if let Ok(pkt) = maybe_pkt {
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
        .map_err(|_| DeviceError::CaptureThreadPanic)??;

    packets_to_trace(&packets, linktype, mac_address)
}

/// Detemine the direction of a packet given the packet data, the [`pcap::Linktype`] (only `ETHERNET`,
/// `LINUX_SLL`, and `LINUX_SLL2` are implemented), and the MAC address (required for `ETHERNET`).
fn determine_packet_direction(
    data: &[u8],
    linktype: Linktype,
    local_mac: [u8; 6],
) -> Result<Direction, DeviceError> {
    match linktype {
        // Reference: https://ieeexplore.ieee.org/document/7428776
        Linktype::ETHERNET => {
            // make sure the bytes we want are there
            if data.len() < 12 {
                return Err(DeviceError::InvalidPacket(
                    "Ethernet frame too short".into(),
                ));
            }

            // header has 6 octets for destination address, followed by 6 bytes for source address
            let src_mac = &data[6..12];

            // if we are operating in promiscuous mode, it is possible that neither source nor
            // destination is us, so just check the source address
            Ok(if src_mac == local_mac {
                Direction::Send
            } else {
                Direction::Receive
            })
        }

        // Reference: https://www.tcpdump.org/linktypes/LINKTYPE_LINUX_SLL.html
        Linktype::LINUX_SLL => {
            // make sure the bytes we want are there
            if data.len() < 2 {
                return Err(DeviceError::InvalidPacket("SLL frame too short".into()));
            }

            let packet_type = u16::from_be_bytes([data[0], data[1]]);
            match packet_type {
                // 0-3 are all sent by someone else
                0..=3 => Ok(Direction::Receive),
                // 4 is sent by us
                4 => Ok(Direction::Send),
                _ => Err(DeviceError::InvalidPacket(format!(
                    "Invalid SLL type: {packet_type}"
                ))),
            }
        }

        // Reference: https://www.tcpdump.org/linktypes/LINKTYPE_LINUX_SLL2.html
        Linktype::LINUX_SLL2 => {
            // make sure the bytes we want are there
            if data.len() < 11 {
                return Err(DeviceError::InvalidPacket("SLL2 frame too short".into()));
            }

            match data[10] {
                // 0-3 are all sent by someone else
                0..=3 => Ok(Direction::Receive),
                // 4 is sent by us
                4 => Ok(Direction::Send),
                _ => Err(DeviceError::InvalidPacket(format!(
                    "Invalid SLL2 type: {}",
                    data[10]
                ))),
            }
        }
        _ => Err(DeviceError::InvalidPacket(format!(
            "Unsupported linktype: {linktype:?}"
        ))),
    }
}

/// Utility function to convert the [`pcap::PacketHeader::ts`] into microseconds.
#[expect(clippy::cast_sign_loss)]
fn packet_ts_to_us(header: PacketHeader) -> u64 {
    let tv = header.ts;
    let sec_us = (tv.tv_sec as u64) * 1_000_000;
    let usec_us = tv.tv_usec as u64;

    sec_us + usec_us
}

/// Converts packet information into traces.
fn packets_to_trace(
    packets: &[(PacketHeader, Vec<u8>)],
    linktype: Linktype,
    mac_address: MacAddress,
) -> Result<Trace, CaptureError> {
    if packets.is_empty() {
        return Ok(Trace {
            directions: Box::default(),
            timing_deltas: Box::default(),
            sizes: Box::default(),
        });
    }

    let mac_address = mac_address.bytes();

    let directions: Box<[Direction]> = packets
        .iter()
        .map(|(_, data)| determine_packet_direction(data, linktype, mac_address))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();

    // Don't expect the time difference between two packets to be this large.
    #[expect(clippy::cast_possible_truncation)]
    let timing_deltas: Box<[u32]> = std::iter::once(0)
        .chain(
            packets
                .windows(2)
                .map(|w| packet_ts_to_us(w[1].0).saturating_sub(packet_ts_to_us(w[0].0)) as u32),
        )
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let sizes: Box<[u32]> = packets
        .iter()
        .map(|pkt| pkt.0.len)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    assert_eq!(
        directions.len(),
        timing_deltas.len(),
        "Length of directions and timing deltas do not match after conversion of packet to trace."
    );
    assert_eq!(
        sizes.len(),
        timing_deltas.len(),
        "Length of sizes and timing deltas do not match after conversion of packet to trace."
    );

    Ok(Trace {
        directions,
        timing_deltas,
        sizes,
    })
}

/// Stream a [`pcap::Capture`] into a [`Trace`] using the provided `local_mac` to determine
/// direction if the [`pcap::Capture`]'s [`pcap::Linktype`] is `ETHERNET`.
pub fn capture_to_trace<T: pcap::Activated>(
    capture: &mut Capture<T>,
    local_mac: MacAddress,
) -> Result<Trace, CaptureError> {
    let linktype = capture.get_datalink();
    let mac_bytes = local_mac.bytes();

    if let Ok(first) = capture.next_packet() {
        let first_ts = packet_ts_to_us(*first.header);
        let mut trace_builder = TraceBuilder::new(first_ts);
        let dir = determine_packet_direction(first.data, linktype, mac_bytes)?;
        trace_builder.record(dir, first_ts, first.header.len);

        while let Ok(packet) = capture.next_packet() {
            let dir = determine_packet_direction(packet.data, linktype, mac_bytes)?;
            let current_ts_us = packet_ts_to_us(*packet.header);
            trace_builder.record(dir, current_ts_us, packet.header.len);
        }

        Ok(trace_builder.build())
    } else {
        Err(CaptureError::Trace(TraceError::UnexpectedEof))
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_ethernet_direction_send() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data
        let mut data = [0u8; 14];
        data[6..12].copy_from_slice(&local_mac);

        // Check
        let dir = determine_packet_direction(&data, Linktype::ETHERNET, local_mac).unwrap();
        assert!(matches!(dir, Direction::Send));
    }

    #[test]
    fn test_ethernet_direction_recv() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data
        let mut data = [0u8; 14];
        data[..6].copy_from_slice(&local_mac);

        let dir = determine_packet_direction(&data, Linktype::ETHERNET, local_mac).unwrap();
        assert!(matches!(dir, Direction::Receive));
    }

    #[test]
    fn test_ethernet_too_short() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data
        let data = [0u8; 6];

        let dir = determine_packet_direction(&data, Linktype::ETHERNET, local_mac);
        assert!(dir.is_err());
    }

    #[test]
    fn test_linux_sll_direction_send() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data with packet type 0x0004
        let data = [0x00, 0x04];

        let dir = determine_packet_direction(&data, Linktype::LINUX_SLL, local_mac).unwrap();
        assert!(matches!(dir, Direction::Send));
    }

    #[test]
    fn test_linux_sll_direction_recv() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data
        let mut data = [0x00, 0x00];

        // Iterate through receive packet types
        for pkt_type in 0u8..=3u8 {
            data[1] = pkt_type;
            let dir = determine_packet_direction(&data, Linktype::LINUX_SLL, local_mac).unwrap();
            assert!(matches!(dir, Direction::Receive));
        }
    }

    #[test]
    fn test_linux_sll_direction_err() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data with packet type 0x0005
        let data = [0x00, 0x05];

        let dir = determine_packet_direction(&data, Linktype::LINUX_SLL, local_mac);
        assert!(dir.is_err());
    }

    #[test]
    fn test_linux_sll_too_short() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        let data = [0x00];
        let dir = determine_packet_direction(&data, Linktype::LINUX_SLL, local_mac);
        assert!(dir.is_err());
    }

    #[test]
    fn test_linux_sll2_direction_send() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data with packet type 0x04
        let mut data = [0; 11];
        data[10] = 0x04;

        let dir = determine_packet_direction(&data, Linktype::LINUX_SLL2, local_mac).unwrap();
        assert!(matches!(dir, Direction::Send));
    }

    #[test]
    fn test_linux_sll2_direction_recv() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data with packet type 0x05
        let mut data = [0; 11];

        // Iterate through receive packet types
        for pkt_type in 0u8..=3u8 {
            data[10] = pkt_type;
            let dir = determine_packet_direction(&data, Linktype::LINUX_SLL2, local_mac).unwrap();
            assert!(matches!(dir, Direction::Receive));
        }
    }

    #[test]
    fn test_linux_sll2_direction_err() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Fake packet data with packet type 0x05
        let mut data = [0; 11];
        data[10] = 0x05;

        let dir = determine_packet_direction(&data, Linktype::LINUX_SLL2, local_mac);
        assert!(dir.is_err());
    }

    #[test]
    fn test_linux_sll2_too_short() {
        // Fake local MAC address
        let local_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        let data = [0; 10];
        let dir = determine_packet_direction(&data, Linktype::LINUX_SLL2, local_mac);
        assert!(dir.is_err());
    }

    /// This test is for [`packets_to_trace()`], but also covers [`packet_ts_to_us()`] and [`determine_packet_direction()`] with `Linktype::ETHERNET`.
    #[test]
    fn test_packets_to_trace_ethernet_from_file() {
        // Construct path to pcap
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-http-5.pcap");

        // Create a capture from the file
        let mut cap = Capture::from_file(path).unwrap();
        let linktype = cap.get_datalink();

        // First packet source
        let local_mac = MacAddress::from([0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);

        let mut packets = Vec::new();
        while let Ok(pkt) = cap.next_packet() {
            packets.push((*pkt.header, pkt.data.to_vec()));
        }

        let trace = packets_to_trace(&packets, linktype, local_mac).unwrap();

        // The following values were found by inspecting the pcap with `tshark`.

        // Since we used the first packet source MAC as the local MAC, this should
        // be send. The second packet is a SYN ACK from the remote.
        assert!(matches!(trace.directions[0], Direction::Send));
        assert!(matches!(trace.directions[1], Direction::Receive));

        assert_eq!(trace.timing_deltas[0], 0);
        assert_eq!(trace.timing_deltas[1], 911_310);

        // `tshark -r ./test-pcaps/test-http-5.pcap -T fields -e frame.len`
        assert_eq!(trace.sizes[0], 62);
        assert_eq!(trace.sizes[1], 62);
        assert_eq!(trace.sizes[3], 533);
    }

    /// This test is for [`packets_to_trace()`], but also covers [`packet_ts_to_us()`] and [`determine_packet_direction()`] with `Linktype::SLL2`.
    #[test]
    fn test_packets_to_trace_sll2_type_0_from_file() {
        // Construct path to pcap
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-sll2-single-type-0.pcap");

        // Create a capture from the file
        let mut cap = Capture::from_file(path).unwrap();
        let linktype = cap.get_datalink();

        // Fake mac
        let local_mac = MacAddress::from([0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe]);

        let mut packets = Vec::new();
        while let Ok(pkt) = cap.next_packet() {
            packets.push((*pkt.header, pkt.data.to_vec()));
        }

        let trace = packets_to_trace(&packets, linktype, local_mac).unwrap();

        // The following values were found by inspecting the pcap with `tshark`.
        assert!(matches!(trace.directions[0], Direction::Receive));
        assert_eq!(trace.timing_deltas[0], 0);
        assert_eq!(trace.sizes[0], 144);
    }

    /// This test is for [`packets_to_trace()`], but also covers [`packet_ts_to_us()`] and [`determine_packet_direction()`] with `Linktype::SLL2`.
    #[test]
    fn test_packets_to_trace_sll2_type_4_from_file() {
        // Construct path to pcap
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-sll2-single-type-4.pcap");

        // Create a capture from the file
        let mut cap = Capture::from_file(path).unwrap();
        let linktype = cap.get_datalink();

        // Fake mac
        let local_mac = MacAddress::from([0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe]);

        let mut packets = Vec::new();
        while let Ok(pkt) = cap.next_packet() {
            packets.push((*pkt.header, pkt.data.to_vec()));
        }

        let trace = packets_to_trace(&packets, linktype, local_mac).unwrap();

        // The following values were found by inspecting the pcap with `tshark`.
        assert!(matches!(trace.directions[0], Direction::Send));
        assert_eq!(trace.timing_deltas[0], 0);
        assert_eq!(trace.sizes[0], 144);
    }

    #[test]
    fn test_packets_to_trace_empty() {
        let packets = [];
        let linktype = Linktype::ETHERNET;
        let local_mac = MacAddress::from([0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe]);
        let trace = packets_to_trace(&packets, linktype, local_mac).unwrap();
        assert_eq!(trace.directions.len(), 0);
    }

    #[test]
    fn test_packets_to_trace_unsupported_linktype() {
        // Construct path to pcap
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-sll2-single-type-0.pcap");

        // Create a capture from the file as dummy data
        let mut cap = Capture::from_file(path).unwrap();

        // Use an unsupported linktype
        let linktype = Linktype::USER0;

        // Fake mac
        let local_mac = MacAddress::from([0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe]);

        let mut packets = Vec::new();
        while let Ok(pkt) = cap.next_packet() {
            packets.push((*pkt.header, pkt.data.to_vec()));
        }

        let trace = packets_to_trace(&packets, linktype, local_mac);

        assert!(trace.is_err());
    }

    #[test]
    fn test_capture_to_trace_success() {
        // Construct path to pcap
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-http-5.pcap");

        // Create a capture from the file as dummy data
        let mut cap = Capture::from_file(path).unwrap();
        let local_mac = MacAddress::from([0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);

        let trace = capture_to_trace(&mut cap, local_mac)
            .expect("should successfully stream pcap to trace");

        assert_eq!(trace.directions.len(), 5);
        assert_eq!(trace.directions[0], Direction::Send);
        assert_eq!(trace.directions[1], Direction::Receive);

        assert_eq!(trace.timing_deltas[0], 0);
        assert_eq!(trace.timing_deltas[1], 911_310);

        assert_eq!(trace.sizes[0], 62);
        assert_eq!(trace.sizes[1], 62);
    }

    #[test]
    fn test_capture_to_trace_sll2() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-sll2-single-type-0.pcap");

        let mut cap = Capture::from_file(path).unwrap();
        let local_mac = MacAddress::from([0; 6]);

        let trace = capture_to_trace(&mut cap, local_mac).unwrap();

        assert_eq!(trace.directions.len(), 1);
        assert_eq!(trace.directions[0], Direction::Receive);
        assert_eq!(trace.sizes[0], 144);
    }

    #[test]
    fn test_capture_to_trace_empty_error() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test-pcaps/test-sll2-single-type-0.pcap");

        let mut cap = Capture::from_file(path).unwrap();

        // Exhaust the capture
        while cap.next_packet().is_ok() {}

        let result = capture_to_trace(&mut cap, MacAddress::from([0; 6]));

        match result {
            Err(CaptureError::Trace(TraceError::UnexpectedEof)) => (),
            _ => panic!("unexpected result: {result:?}"),
        }
    }
}
