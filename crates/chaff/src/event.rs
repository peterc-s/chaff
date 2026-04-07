//! Chaff events.

// Not testable, manually tested.
#![cfg(not(tarpaulin_include))]

/// An event that can be emitted either by the integrator or by the framework.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Event {
    /// Normal packet sent (egress). Emitted by integrator.
    SendNormal,

    /// Normal packet received (ingress). Emitted by integrator.
    ReceiveNormal,

    /// Given queue was popped. Emitted by framework.
    QueuePopped(u8),
}

impl Event {
    /// Whether an event can be emitted by a [`crate::framework::Framework`] as deferred.
    pub fn is_deferred(&self) -> bool {
        match self {
            Self::SendNormal | Self::ReceiveNormal => false,
            Self::QueuePopped(_) => true,
        }
    }
}
