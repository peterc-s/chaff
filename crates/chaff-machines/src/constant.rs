//! An attempted implementation of a machine that tries to make packets flow at a constant rate.

#![cfg(not(tarpaulin_include))]

use std::time::Duration;

use chaff::{
    action::{FrameworkAction, IntegratorAction},
    distr::Distr,
    event::Event,
    machine,
    machine::Machine,
};

/// Construct the constant machine.
///
/// # Panics
///
/// Should not panic unless modified.
#[must_use]
#[expect(clippy::expect_used)]
pub fn construct() -> Machine {
    let hundred_ms: Distr = Duration::from_millis(100)
        .try_into()
        .expect("constant shouldn't cause validation issue");
    let max: Distr = Duration::from_nanos(u64::MAX)
        .try_into()
        .expect("constant shouldn't cause validation issue");
    let zero: Distr = Duration::ZERO
        .try_into()
        .expect("constant shouldn't cause validation issue");

    machine! {
        queues: [Some(1), Some(1), Some(1)],
        budget: Absolute(500),
        state init {
            action: IntegratorAction::BlockOutgoing(max),
            transitions: [
                Event::SendNormal => init,
                Event::SendBlocked => schedule_release,
                Event::ReceiveNormal => schedule_release,
            ],
        },
        state do_block {
            action: FrameworkAction::schedule(
                IntegratorAction::BlockOutgoing(max),
                1,
                zero,
            ),
            transitions: [
                Event::QueueFilled(1) => schedule_release,
            ],
        },
        state send_decoy {
            action: FrameworkAction::schedule(
                IntegratorAction::SendDecoy,
                2,
                zero,
            ),
            transitions: [
                Event::QueueFilled(2) => do_block,
            ]
        },
        state schedule_release {
            action: FrameworkAction::schedule(
                IntegratorAction::ReleaseBlock,
                0,
                hundred_ms,
            ),
            transitions: [
                Event::QueueFilled(0) => blocked,
            ],
        },
        state blocked {
            transitions: [
                Event::QueueEmpty(0) => send_decoy,
                Event::SendBlocked => wait,
            ]
        },
        state wait {
            transitions: [
                Event::QueueEmpty(0) => do_block,
            ]
        },
    }
    .expect("predefined machine should be valid")
}
