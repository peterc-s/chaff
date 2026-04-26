//! Chaff actions for integrators ([`IntegratorAction`]) and the framework ([`FrameworkAction`]) to
//! take.

use crate::distr::Distr;

/// Actions the framework should take.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Debug, Clone, PartialEq)]
pub enum FrameworkAction {
    /// Schedule an [`IntegratorAction`] to expire after a delay on the given queue.
    Schedule {
        /// The [`IntegratorAction`] to schedule.
        action: IntegratorAction,

        /// The queue to schedule on.
        queue: u8,

        /// The delay for scheduling.
        delay: Box<Distr>,
    },

    /// Cancel all actions on a given queue.
    CancelQueue(u8),

    /// Cancel all actions on all queues.
    CancelAll,
}

impl FrameworkAction {
    /// Create a new [`FrameworkAction::Schedule`] with given action, queue, and delay.
    pub fn schedule(action: impl Into<IntegratorAction>, queue: u8, delay: Distr) -> Self {
        Self::Schedule {
            action: action.into(),
            queue,
            delay: Box::new(delay),
        }
    }
}

/// Actions an integrator should take.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegratorAction {
    /// Send a decoy packet.
    SendDecoy,

    /// Block outgoing packets for the given duration.
    BlockOutgoing(Distr),

    /// Release any existing block on outgoing packets.
    ReleaseBlock,
}

impl IntegratorAction {
    /// Ergonomic constructor for [`IntegratorAction::BlockOutgoing`].
    #[must_use]
    pub fn block_outgoing(delay: Distr) -> Self {
        Self::BlockOutgoing(delay)
    }
}

/// An enum with each of the actions Chaff requires an integrator to make.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Debug, Clone, PartialEq)]
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
