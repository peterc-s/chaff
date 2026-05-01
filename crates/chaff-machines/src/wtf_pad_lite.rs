//! The "WTF-PAD lite" defence. Simplified version of WTF-PAD for demonstration purposes.
//!
//! This defence does not do both burst and gap level padding, has different stop conditions, and a
//! different way of sampling distributions.

#![cfg(not(tarpaulin_include))]

use chaff::{
    action::{FrameworkAction, IntegratorAction},
    distr::Distr,
    event::Event,
    machine,
    machine::Machine,
};

/// Constructs a "WTF-PAD lite" defence. A simplified version of WTF-PAD for demonstration purposes.
///
/// This defence does not do both burst and gap level padding, has different stop conditions, and a
/// different way of sampling distributions.
/// # Panics
///
/// Shouldn't panic unless modified.
#[expect(clippy::expect_used)]
pub fn construct(
    delay_distr: impl Into<Distr> + Copy,
    timeout: impl Into<Distr> + Copy,
) -> Machine {
    machine! {
        queues: [None, None],
        budget: Absolute(500),
        state init {
            transitions: [
                Event::SendNormal => active,
                Event::ReceiveNormal => active,
            ]
        },
        state active {
            actions: [
                FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    delay_distr.into()
                ),
                FrameworkAction::schedule(
                    IntegratorAction::ReleaseBlock,
                    1,
                    timeout.into()
                ),
            ],
            transitions: [
                // if we send or receive anything, reset the timer and note
                // that we received real traffic
                Event::SendNormal => active_real,
                Event::ReceiveNormal => active_real,

                // if we haven't sent or received anything, then a decoy
                // must have been sent, so reset the delay timer
                Event::QueuePopped(0) => active,

                // if the timeout happens, safely exit.
                Event::QueuePopped(1) => end,

                // if we exhaust the budget, safely exit
                Event::MachineBudgetExhausted => end,
            ],
        },
        state active_real {
            actions: [
                FrameworkAction::CancelAll,
                FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    delay_distr.into()
                ),
                FrameworkAction::schedule(
                    IntegratorAction::ReleaseBlock,
                    1,
                    timeout.into()
                ),
            ],
            transitions: [
                // if we send or receive anything, reset the timer and
                // note that we received real traffic
                Event::SendNormal => active_real,
                Event::ReceiveNormal => active_real,

                // if we haven't sent or received anything, then a decoy
                // must have been sent, so reset the timer in active
                Event::QueuePopped(0) => active,

                Event::QueuePopped(1) => end,
                Event::MachineBudgetExhausted => end,
            ],
        },
        state end {
            actions: [FrameworkAction::CancelAll],
        }
    }
    .expect("preconstructed machine should be valid")
}
