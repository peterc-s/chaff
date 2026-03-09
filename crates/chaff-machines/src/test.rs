//! Test machine used while developing the Chaff framework.

use chaff::framework::{Action, Event, Machine, State, Transition, TransitionProbs};

/// Construct the test machine.
pub fn construct_test_machine() -> Machine {
    let trans_probs = TransitionProbs::from_fn(|event| match event {
        Event::SendNormal => Some(Transition(1, 0.5)),
        Event::ReceiveNormal => None,
    });

    Machine::new(vec![
        State::new(Some(trans_probs), Action::SendDecoy),
        State::new(None, Action::SendDecoy),
    ])
}
