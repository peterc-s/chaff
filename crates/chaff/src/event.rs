//! Chaff events.

// Not testable, manually tested.
#![cfg(not(tarpaulin_include))]

/// An event that can be emitted either by the integrator or by the framework.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Event {
    /// Normal packet sent (egress). Emitted by integrator.
    SendNormal,

    /// Normal packet received (ingress). Emitted by integrator.
    ReceiveNormal,

    /// Decoy packet sent (egress). Emitted by integrator.
    SendDecoy,

    /// Decoy packet received (ingress). Emitted by integrator.
    ReceiveDecoy,

    /// Given queue was popped. Emitted by framework.
    QueuePopped(u8),

    /// Given queue has reached capacity. Emitted by framework.
    QueueFull(u8),

    /// Given queue has emptied. Emitted by framework.
    QueueEmpty(u8),

    /// The current state budget has been exhausted. Emitted by framework.
    StateBudgetExhausted,

    /// The machine's budget has been exhausted and cannot recover. Emitted by framework.
    MachineBudgetExhausted,

    /// The machine's budget has been reached for now, but could recover (for example, because
    /// [`crate::machine::MachineDecoyBudget::Proportion`] is being used). Emitted by framework.
    MachineBudgetReached,

    /// The machine's budget was previously reached but there is now budget. Emitted by framework.
    MachineBudgetRecovered,
}

impl Event {
    /// Whether an event can be emitted by a [`crate::framework::Framework`] as deferred.
    #[must_use]
    pub fn is_deferred(&self) -> bool {
        match self {
            Self::SendNormal | Self::ReceiveNormal | Self::SendDecoy | Self::ReceiveDecoy => false,
            Self::QueuePopped(_)
            | Self::QueueFull(_)
            | Self::StateBudgetExhausted
            | Self::MachineBudgetExhausted
            | Self::MachineBudgetReached
            | Self::MachineBudgetRecovered
            | Self::QueueEmpty(_) => true,
        }
    }
}
