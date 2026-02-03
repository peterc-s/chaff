- `test-http-5.pcap`
    - Source: [Sample Captures](https://wiki.wireshark.org/uploads/27707187aeb30df68e70c8fb9d614981/http.cap)
    - Command used: `tshark -r http.cap -Y "frame.number <= 5" -w test-http-5.pcap`

