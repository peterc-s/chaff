//! The different representations of traces and methods for working with traces.

/// Packet direction.
#[derive(Debug)]
pub enum Direction {
    /// From client to server.
    Send,

    /// From server to client.
    Receive,
}

/// Represents a trace explicitly with packet [Direction]s, packet timing deltas, and sizes
/// (assumes non-fixed transmission unit size). Fixed-size
#[derive(Debug)]
pub struct Trace {
    /// The direction in which a packet was send.
    pub directions: Box<[Direction]>,

    /// How long between last packet and this one.
    pub timing_deltas: Box<[u64]>,

    /// Assuming largest MTU is 4GiB (IPv6 jumbograms, for example).
    pub sizes: Box<[u32]>,
}
