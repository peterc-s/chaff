//! Chaff actions for integrators to take.

use std::time::Duration;

/// Actions the framework should take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameworkAction {
    /// Schedule an [`IntegratorAction`] to expire after a delay on the given queue.
    Schedule {
        /// The [`IntegratorAction`] to schedule.
        action: IntegratorAction,

        /// The queue to schedule on.
        queue: u8,

        /// The delay for scheduling.
        delay: Duration,
    },

    /// Cancel all actions on a given queue.
    CancelQueue(u8),

    /// Cancel all actions on all queues.
    CancelAll,
}

/// Actions an integrator should take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegratorAction {
    /// Send a decoy packet.
    SendDecoy,

    /// Block outgoing packets for the given duration.
    BlockOutgoing(Duration),

    /// Release any existing block on outgoing packets.
    ReleaseBlock,
}

/// An enum with each of the actions Chaff requires an integrator to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// An action for the framework to take, see [`FrameworkAction`].
    Framework(FrameworkAction),

    /// An action for the integrator to take, see [`IntegratorAction`].
    Integrator(IntegratorAction),
}

impl From<FrameworkAction> for Action {
    fn from(value: FrameworkAction) -> Self {
        Self::Framework(value)
    }
}

impl From<IntegratorAction> for Action {
    fn from(value: IntegratorAction) -> Self {
        Self::Integrator(value)
    }
}
