//! The Chaff framework.
// TODO: document

#![expect(dead_code)]
#![expect(missing_docs)]

#[derive(Default)]
pub struct Framework {
    machine: Machine,
}

impl Framework {
    pub fn new(machine: Machine) -> Self {
        Self { machine }
    }
}

#[derive(Default)]
pub struct Machine {
    states: Vec<State>,
}

impl Machine {
    pub fn new(states: Vec<State>) -> Self {
        Self { states }
    }
}

#[derive(Default)]
pub struct State {
    transitions: Option<Vec<Transition>>,
}

impl State {
    pub fn new(transitions: Option<Vec<Transition>>) -> Self {
        Self { transitions }
    }
}

pub struct Transition(pub usize, pub f32);
