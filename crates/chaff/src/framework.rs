//! The Chaff framework.
// TODO: document

use std::time::Instant;

use rand::Rng;

use crate::{
    action::Action,
    event::Event,
    machine::{Machine, MachineRuntime},
    state::TransitionProbs,
};

/// Represents an instance of the Chaff framework.
#[derive(Default, Debug, Clone)]
pub struct Framework<R: Rng> {
    pub(crate) machine: Machine,
    pub(crate) runtime: MachineRuntime,
    rng: R,
}

impl<R: Rng> Framework<R> {
    /// Create a new Chaff instance with the given RNG ([`rand::Rng`]) and [`Machine`].
    pub fn new(machine: Machine, rng: R) -> Self {
        let runtime = MachineRuntime::new(&machine);
        Self {
            machine,
            runtime,
            rng,
        }
    }

    fn get_trans_probs(&self) -> Option<TransitionProbs> {
        // TODO: this path can't be covered yet, an out-of-range index causes a panic in trigger_events, which should instead be an error
        // but this means adding errors to the main crate, which I'll do later.
        self.machine
            .states
            .get(self.runtime.state)
            .map(|state| state.trans_probs)?
    }

    /// "Trigger" a slice of events, returns actions the integrator must take.
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

    /// Pops the [`MachineRuntime`]'s action queues giving a list of actions that the integrator
    /// must take.
    pub fn pop_queues(&mut self, now: Instant) -> Box<[Action]> {
        self.runtime.pop_queues(now)
    }

    /// First pops the queues with [`Framework::pop_queues()`], then triggers the given events with [`Framework::trigger_events()`],
    /// returning the concatenation of the resulting [`Box<T>`]'s of [`Action`] slices.
    pub fn trigger_events_and_pop_queues(
        &mut self,
        events: &[Event],
        now: Instant,
    ) -> Box<[Action]> {
        self.pop_queues(now)
            .into_iter()
            .chain(self.trigger_events(events))
            .collect()
    }

    /// Get the current state of the frameworks machine.
    pub fn get_state(&self) -> usize {
        self.runtime.state
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        action::Action,
        event::Event,
        machine::Machine,
        state::{State, TransitionProbs},
    };

    #[test]
    fn test_get_trans_probs() {
        let trans_probs = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((1, 0.5).into()),
            Event::ReceiveNormal => None,
        });
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), Action::SendDecoy),
                State::new(None, Action::SendDecoy),
            ],
            0,
        );
        let framework = Framework::new(machine, rand::rng());

        assert_eq!(framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_and_get_trans_probs() {
        let trans_probs = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((1, 1.0).into()),
            Event::ReceiveNormal => None,
        });
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), Action::SendDecoy),
                State::new(None, Action::SendDecoy),
            ],
            0,
        );
        let mut framework = Framework::new(machine, rand::rng());

        assert_eq!(framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);

        framework.trigger_events(&[Event::SendNormal]);

        assert!(framework.get_trans_probs().is_none());
        assert_eq!(framework.get_state(), 1);
    }

    #[test]
    fn test_trigger_with_0_trans_probs() {
        let trans_probs = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((1, 0.0).into()),
            Event::ReceiveNormal => None,
        });
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), Action::SendDecoy),
                State::new(None, Action::SendDecoy),
            ],
            0,
        );
        let mut framework = Framework::new(machine, rand::rng());

        assert_eq!(framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);

        framework.trigger_events(&[Event::SendNormal]);

        assert_eq!(framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);
    }

    // #[test]
    // fn test_invalid_state_transition() {
    //     let trans_probs = TransitionProbs::from_fn(|_| Some((99, 1.0).into()));
    //     let machine = Machine::new(vec![State::new(Some(trans_probs), Action::SendDecoy)], 0);
    //     let mut framework = Framework::new(machine, rand::rng());
    //
    //     framework.trigger_events(&[Event::SendNormal]);
    //
    //     assert!(framework.get_trans_probs().is_none());
    // }
}
