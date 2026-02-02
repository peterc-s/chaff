//! Use [libpcap](https://github.com/the-tcpdump-group/libpcap) through the [pcap] crate to capture
//! a [`crate::trace::Trace`].

use pcap::{Capture, Device};

/// TEMP: testing capture
pub fn test_capture() {
    // TODO: wrap this nicely, handle all the errors, note that this needs to be run as `root` if
    // the capabilities of the binary isn't set. See [this note](https://github.com/rust-pcap/pcap/blob/23d1752b45accf20827c2dde80d8a36fcee16233/README.md?plain=1#L44).
    #![allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    let mut cap = Capture::from_device(Device::lookup().unwrap().unwrap())
        .unwrap()
        .open()
        .unwrap();

    while let Ok(packet) = cap.next_packet() {
        println!("Received {packet:?}");
    }
}
