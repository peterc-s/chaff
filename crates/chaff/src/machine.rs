//! Chaff machines

use std::{borrow::Borrow, time::Instant};

use crate::{
    action::Action,
    errors::ValidationError,
    queue::{TimedAction, TimedQueue},
    state::State,
};

/// The Chaff machine specification.
#[derive(Default, Debug, Clone)]
pub struct Machine {
    pub(crate) states: Vec<State>,
    pub(crate) queues: u8,
}

impl Machine {
    /// Create a new [`Machine`] with the given states.
    pub fn new(states: Vec<State>, queues: u8) -> Result<Self, ValidationError> {
        // find transitions with indices exceeding the number of states
        let num_states = states.len();
        let invalid_transitions = states
            .iter()
            .filter_map(|state| state.trans_probs.as_ref())
            .flat_map(|probs| probs.0.iter().flatten())
            .map(|trans| trans.index)
            .filter(|&index| index > num_states)
            .collect::<Vec<_>>();

        if invalid_transitions.is_empty() {
            Ok(Self { states, queues })
        } else {
            Err(ValidationError::TransitionToInvalidState(
                invalid_transitions.into_boxed_slice(),
            ))
        }
    }
}

/// The runtime for a machine.
#[derive(Default, Debug, Clone)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    pub(crate) state: usize,

    /// Vector of priority queues corresponding to [`Machine::queues`].
    pub(crate) queues: Vec<TimedQueue<TimedAction>>,
}

impl MachineRuntime {
    /// Create a new [`MachineRuntime`] for a given [`Machine`].
    pub fn new<M: Borrow<Machine>>(machine: M) -> Self {
        let m = machine.borrow();
        let queues = (0..m.queues).map(|_| TimedQueue::new()).collect();
        Self { state: 0, queues }
    }

    /// Pops the action [`TimedQueues`].
    pub fn pop_queues(&mut self, now: Instant) -> Box<[Action]> {
        let mut actions = Vec::new();

        for queue in &mut self.queues {
            actions.extend(queue.pop_ready(now));
        }

        actions
            .iter()
            .map(|timed_action| timed_action.action)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::{event::Event, framework::Framework, state::TransitionProbs};

    use super::*;

    #[test]
    fn test_queues_correct_len() {
        let trans_probs = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((1, 0.0).into()),
            Event::ReceiveNormal => None,
        })
        .unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), Action::SendDecoy),
                State::new(None, Action::SendDecoy),
            ],
            42,
        )
        .unwrap();
        let framework = Framework::new(machine, rand::rng());

        assert_eq!(
            framework.runtime.queues.len(),
            framework.machine.queues as usize
        );
    }

    #[test]
    fn test_pop_queues_with_data() {
        let machine = Machine::new(vec![], 1).unwrap();
        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        framework.runtime.queues[0].push(TimedAction {
            action: Action::SendDecoy,
            execute_at: now,
        });

        let actions = framework.pop_queues(now);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], Action::SendDecoy);
    }

    #[test]
    fn test_validate_invalid_state() {
        let trans_probs = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((3, 0.0).into()),
            Event::ReceiveNormal => None,
        })
        .unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), Action::SendDecoy),
                State::new(None, Action::SendDecoy),
            ],
            42,
        );

        match machine {
            Err(ValidationError::TransitionToInvalidState(state)) => {
                assert!(state[0] == 3usize, "unexpected invalid state: {state:?}");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
