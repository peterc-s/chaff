//! Chaff machines

use std::{borrow::Borrow, time::Instant};

use crate::{
    action::{Action, FrameworkAction},
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
    /// Validates given states and number of queues:
    /// - Transitions must not go to invalid states.
    /// - State actions must not interact with non-existent queues.
    ///
    /// # Panics
    ///
    /// This should not panic under any normal circumstances. This method contains a [`Result::expect`]
    /// which should be safe because of a prior bounds check.
    fn validate(states: &[State], queues: u8) -> Result<(), ValidationError> {
        let mut errors = vec![];
        let num_states = states.len();

        // check for transitions to invalid states
        let invalid_transitions = states
            .iter()
            .filter_map(|state| state.trans_probs.as_ref())
            .flat_map(|probs| probs.0.values())
            .map(|trans| trans.index)
            .filter(|&index| index > num_states)
            .collect::<Vec<_>>();

        if !invalid_transitions.is_empty() {
            errors.push(ValidationError::TransitionToInvalidState(
                invalid_transitions.into_boxed_slice(),
            ));
        }

        // check for state actions which would try to interact with
        // non-existent queues
        let invalid_state_action_queues = states
            .iter()
            .map(|state| state.action)
            .filter_map(|action| match action {
                Action::Framework(framework_action) => match framework_action {
                    FrameworkAction::Schedule { queue, .. }
                    | FrameworkAction::CancelQueue(queue)
                        if queue > queues =>
                    {
                        Some(queue)
                    }
                    _ => None,
                },
                Action::Integrator(_) => None,
            })
            .collect::<Vec<_>>();

        if !invalid_state_action_queues.is_empty() {
            errors.push(ValidationError::InvalidStateActionQueue(
                invalid_state_action_queues.into_boxed_slice(),
            ));
        }

        #[expect(clippy::expect_used)]
        match errors.len() {
            0 => Ok(()),
            // SAFETY: expect here is okay as the length matching means we must have at least one error.
            1 => Err(errors.pop().expect("no errors when popping")),
            _ => Err(ValidationError::Multiple(errors.into_boxed_slice())),
        }
    }

    /// Create a new [`Machine`] with the given states.
    pub fn new(states: Vec<State>, queues: u8) -> Result<Self, ValidationError> {
        Self::validate(&states, queues)?;
        Ok(Self { states, queues })
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
    use std::time::Duration;

    use crate::{
        action::IntegratorAction, event::Event, framework::Framework, state::TransitionProbs,
    };

    use super::*;

    #[test]
    fn test_queues_correct_len() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 0.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(None, IntegratorAction::SendDecoy.into()),
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
            action: IntegratorAction::SendDecoy.into(),
            execute_at: now,
        });

        let actions = framework.pop_queues(now);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], IntegratorAction::SendDecoy.into());
    }

    #[test]
    fn test_validate_invalid_state() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (3, 0.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(None, IntegratorAction::SendDecoy.into()),
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

    #[test]
    fn test_validate_good_queues() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), FrameworkAction::CancelQueue(1).into()),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 1,
                        delay: Duration::from_secs(1),
                    }
                    .into(),
                ),
            ],
            1,
        );

        assert!(machine.is_ok());
    }

    #[test]
    fn test_validate_invalid_state_action_queue() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), FrameworkAction::CancelQueue(2).into()),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 3,
                        delay: Duration::from_secs(1),
                    }
                    .into(),
                ),
            ],
            1,
        );

        let invalid_queues = vec![2, 3];
        match machine {
            Err(ValidationError::InvalidStateActionQueue(queues)) => {
                assert_eq!(
                    queues,
                    invalid_queues.into_boxed_slice(),
                    "unexpected invalid queues: {queues:?}"
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_multiple_validation_errors() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (5, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), FrameworkAction::CancelQueue(2).into()),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 3,
                        delay: Duration::from_secs(1),
                    }
                    .into(),
                ),
            ],
            1,
        );

        match machine {
            Err(ValidationError::Multiple(errors)) => {
                let num_errors = errors.len();
                assert_eq!(num_errors, 2, "unexpected number of errors: {num_errors}");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
