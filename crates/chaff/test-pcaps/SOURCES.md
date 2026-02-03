- `test-http-5.pcap`
    - Source: [Sample Captures](https://wiki.wireshark.org/uploads/27707187aeb30df68e70c8fb9d614981/http.cap)
    - Command used: `tshark -r http.cap -Y "frame.number <= 5" -w test-http-5.pcap`
- `test-sll2-single-type-0`
    - Source: Captured on a device I own with `tcpdump`, modified to just one frame with `tshark`,
        MAC redacted.
- `test-sll2-single-type-4`
    - Source: Captured on a device I own with `tcpdump`, modified to just one frame with `tshark`,
        MAC redacted.
