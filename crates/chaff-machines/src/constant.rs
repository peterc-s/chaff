//! A machine that sends bursts at a constant rate.

#![cfg(not(tarpaulin_include))]

use std::time::Duration;

use chaff::{action::IntegratorAction, distr::Distr, event::Event, machine, machine::Machine};

/// Construct the constant machine.
///
/// # Panics
///
/// Should not panic unless modified.
#[must_use]
#[expect(clippy::unwrap_used, clippy::expect_used)]
pub fn construct() -> Machine {
    let hundred_ms: Distr = Duration::from_millis(100).try_into().unwrap();

    machine! {
        queues: [],
        budget: Proportion(3.5),

        state wait_clean {
            actions: [
                IntegratorAction::BlockOutgoing(hundred_ms),
            ],
            transitions: [
                Event::SendBlocked => wait_dirty,
                Event::BlockReleased => send_decoy,
                Event::MachineBudgetReached => end,
            ]
        },

        state wait_dirty {
            transitions: [
                Event::BlockReleased => wait_clean,
                Event::MachineBudgetReached => end,
            ]
        },

        state send_decoy {
            actions: [
                IntegratorAction::SendDecoy,
                IntegratorAction::BlockOutgoing(hundred_ms),
            ],
            transitions: [
                Event::SendBlocked => wait_dirty,
                Event::BlockReleased => send_decoy,
                Event::MachineBudgetReached => end,
            ]
        },

        state end {
            actions: [
                IntegratorAction::ReleaseBlock
            ],
            transitions: [
                Event::MachineBudgetRecovered => wait_clean,
            ],
        }
    }
    .expect("preconstructed machine should be valid")
}
