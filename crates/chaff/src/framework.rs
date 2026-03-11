//! The Chaff framework.
// TODO: document

use rand::Rng;

use crate::{
    action::Action,
    event::Event,
    machine::{Machine, MachineRuntime},
    state::TransitionProbs,
};

#[derive(Default, Debug, Clone)]
pub struct Framework<R: Rng> {
    machine: Machine,
    runtime: MachineRuntime,
    rng: R,
}

impl<R: Rng> Framework<R> {
    pub fn new(machine: Machine, rng: R) -> Self {
        Self {
            machine,
            runtime: MachineRuntime::default(),
            rng,
        }
    }

    fn get_trans_probs(&self) -> Option<TransitionProbs> {
        self.machine
            .states
            .get(self.runtime.state)
            .map(|state| state.trans_probs)?
    }

    pub fn trigger_events(&mut self, events: &[Event]) -> Box<[Action]> {
        let mut resulting_actions = vec![];

        for event in events {
            if let Some(trans_probs) = self.get_trans_probs() {
                if let Some(new_state) = trans_probs.trigger(&mut self.rng, *event) {
                    self.runtime.state = new_state;
                    resulting_actions.push(self.machine.states[new_state].action);
                }
            }
        }

        resulting_actions.into_boxed_slice()
    }

    pub fn get_state(&self) -> usize {
        self.runtime.state
    }
}
