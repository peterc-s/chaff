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
    let hundred_ms: Distr = Duration::from_millis(100).try_into().expect("valid distr");
    let long_delay: Distr = Duration::from_micros(u64::MAX)
        .try_into()
        .expect("valid distr");
    let zero: Distr = Duration::ZERO.try_into().expect("valid distr");

    machine! {
        queues: [Some(1), Some(1), Some(1)],
        budget: Absolute(500),
        state init {
            action: IntegratorAction::BlockOutgoing(long_delay), // block immediately
            transitions: [
                Event::SendBlocked => schedule_release, // first blocked packet
            ],
        },
        state schedule_release {
            // schedule a release with a 100ms delay
            action: FrameworkAction::schedule(
                IntegratorAction::ReleaseBlock,
                0,
                hundred_ms,
            ),
            transitions: [
                // since we scheduled on a 1 capacity queue, this transition should
                // happen immediately
                Event::QueueFilled(0) => blocked,
            ],
        },
        state blocked {
            transitions: [
                Event::QueueEmpty(0) => send_decoy, // no real packet sent
                Event::SendBlocked => wait,         // real packet sent
            ]
        },
        state wait {
            transitions: [
                Event::QueueEmpty(0) => do_block, // wait until release happens
            ]
        },
        state send_decoy {
            action: FrameworkAction::schedule(
                IntegratorAction::SendDecoy,
                2,
                zero,
            ),
            transitions: [
                // similar trick to schedule_release, this time used to do two actions
                // in quick succession
                Event::QueueFilled(2) => do_block,
            ]
        },
        state do_block {
            action: FrameworkAction::schedule(
                IntegratorAction::BlockOutgoing(long_delay),
                1,
                zero,
            ),
            transitions: [
                // similar trick to send_decoy
                Event::QueueFilled(1) => schedule_release,
            ],
        },
    }
    .expect("preconstructed machine should be valid")
}
