//! The Chaff framework.
// TODO: document

#![expect(dead_code)]
#![expect(missing_docs)]

#[derive(Default)]
pub struct Framework {
    machine: Machine,
}

#[derive(Default)]
pub struct Machine {
    states: Vec<State>,
}

#[derive(Default)]
pub struct State {
    transitions: Option<Vec<Transition>>,
}

pub struct Transition(pub usize, pub f32);
