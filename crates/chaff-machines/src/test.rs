//! Test machine used while developing the Chaff framework.

// only used for testing and debugging.
#![cfg(not(tarpaulin_include))]

use chaff::{
    action::IntegratorAction,
    event::Event,
    machine::Machine,
    state::{State, TransitionProbs},
};

/// Construct the test machine.
///
/// # Panics
/// - If the `trans_probs` it constructs internally has been modified to be invalid.
/// - If the machine it constructs contains a transition to a state that doesn't exist.
#[expect(clippy::expect_used)]
#[expect(clippy::unwrap_used)]
#[must_use]
pub fn construct_test_machine() -> Machine {
    let trans_probs = TransitionProbs::new([(Event::SendNormal, [(1, 0.5).try_into().unwrap()])])
        .expect("Transition probabilities are invalid.");

    Machine::new(
        vec![
            State::new(Some(trans_probs), Some(IntegratorAction::SendDecoy), None),
            State::new(None, Some(IntegratorAction::SendDecoy), None),
        ],
        [None],
    )
    .expect("Machine contains a transition to a state that doesn't exist.")
}
