//! Test machine used while developing the Chaff framework.

use chaff::{
    action::Action,
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
pub fn construct_test_machine() -> Machine {
    let trans_probs = TransitionProbs::from_fn(|event| match event {
        Event::SendNormal => Some((1, 0.5).into()),
        Event::ReceiveNormal => None,
    })
    .expect("Transition probabilities are invalid.");

    Machine::new(
        vec![
            State::new(Some(trans_probs), Action::SendDecoy),
            State::new(None, Action::SendDecoy),
        ],
        0,
    )
    .expect("Machine contains a transition to a state that doesn't exist.")
}
