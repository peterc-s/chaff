//! The Chaff [`Machine`].

use std::{borrow::Borrow, time::Instant};

use crate::{
    action::{Action, FrameworkAction},
    errors::ValidationError,
    event::Event,
    queue::{TimedAction, TimedQueue},
    state::State,
};

/// The Chaff machine specification. Represents a queue automata with [`State`]s and
/// [`TimedQueue`]s.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Machine {
    pub(crate) states: Vec<State>,
    pub(crate) queues: Vec<Option<usize>>,
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
    fn validate(states: &[State], queues: usize) -> Result<(), ValidationError> {
        let Ok(queues) = u8::try_from(queues) else {
            return Err(ValidationError::TooManyQueues(queues));
        };

        let mut errors = vec![];
        let num_states = states.len();

        // check for transitions to invalid states
        let invalid_transitions = states
            .iter()
            .filter_map(|state| state.trans_probs.as_ref())
            .flat_map(|probs| probs.0.values())
            .flat_map(|transitions| transitions.iter().map(|transition| transition.index))
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
            .map(|state| state.action.clone())
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

    /// Create a new [`Machine`] with the given states and queues with given capacities ([`None`] for
    /// unlimited capacity).
    ///
    /// # Errors
    ///
    /// Errors with a [`ValidationError`] if any validation fails. This may be because there are
    /// invalid transitions ([`ValidationError::TransitionToInvalidState`]), because there are state
    /// actions that would try to interact with non-existent queues
    /// ([`ValidationError::InvalidStateActionQueue`]), or both ([`ValidationError::Multiple`]).
    pub fn new(
        states: Vec<State>,
        queues: impl Into<Vec<Option<usize>>>,
    ) -> Result<Self, ValidationError> {
        let queues = queues.into();
        Self::validate(&states, queues.len())?;
        Ok(Self { states, queues })
    }
}

/// Create a machine with named states.
///
/// # Example
///
/// ```rust
/// use chaff::{
///     machine,
///     action::IntegratorAction,
///     event::Event,
///     machine::Machine,
///     state::{State, TransitionProbs}
/// };
///
/// let machine_macro = machine! {
///     queues: [],
///     
///     state init {
///         action: IntegratorAction::SendDecoy,
///         budget: 25,
///         transitions: [
///             Event::SendNormal => [(end, 0.5)],
///             Event::ReceiveNormal => end
///         ],
///     },
///     
///     state end {
///         action: IntegratorAction::SendDecoy
///     }
/// }.unwrap();
///
/// let machine_manual = Machine::new(
///     vec![
///         State::new(
///             Some(TransitionProbs::from_tuples([
///                 (Event::SendNormal, [(1, 0.5)]),
///                 (Event::ReceiveNormal, [(1, 1.0)])
///             ]).unwrap()),
///             IntegratorAction::SendDecoy,
///             Some(25),
///         ),
///         State::new(None, IntegratorAction::SendDecoy, None)
///     ],
///     [],
/// ).unwrap();
///
/// assert_eq!(machine_macro, machine_manual);
/// ```
///
/// # Compile-Time Errors
///
/// The `action:` field is strictly required for every state. Omitting it will cause a compile-time
/// error.
///
/// ```rust,compile_fail
/// use chaff::{machine, event::Event, action::IntegratorAction};
///
/// let machine = machine! {
///     queues: [],
///
///     state missing_action {
///         transitions: [
///             Event::SendNormal => end,
///         ],
///         budget: 25,
///     },
///
///     state end {
///         action: IntegratorAction::SendDecoy
///     }
/// }
/// ```
#[macro_export]
macro_rules! machine {
    // transition targets
    (@targets [ $( ($target:ident, $prob:expr) ),* $(,)? ]) => {
        vec![ $( ($target, $prob) ),* ]
    };
    (@targets $target:ident) => {
        vec![($target, 1.0)]
    };

    // state field parsing
    // parse `action:` - updates the status to found
    (@parse_state $a:ident $t:ident $b:ident [$($status:tt)*] action: $action:expr $(, $($rest:tt)*)? ) => {
        $a = ::core::option::Option::Some($action);
        $crate::machine!(@parse_state $a $t $b [found] $($($rest)*)?);
    };

    // parse `transitions:`
    (@parse_state $a:ident $t:ident $b:ident [$($status:tt)*] transitions: [ $( $event:expr => $targets:tt ),* $(,)? ] $(, $($rest:tt)*)? ) => {
        $t = ::core::option::Option::Some(
            $crate::state::TransitionProbs::from_tuples([
                $( ($event, $crate::machine!(@targets $targets)) ),*
            ])?
        );
        $crate::machine!(@parse_state $a $t $b [$($status)*] $($($rest)*)?);
    };

    // parse `budget:`
    (@parse_state $a:ident $t:ident $b:ident [$($status:tt)*] budget: $budget:expr $(, $($rest:tt)*)? ) => {
        $b = ::core::option::Option::Some($budget);
        $crate::machine!(@parse_state $a $t $b [$($status)*] $($($rest)*)?);
    };

    // base case, `action:` found
    (@parse_state $a:ident $t:ident $b:ident [found]) => {};

    // base case, `action:` not found
    (@parse_state $a:ident $t:ident $b:ident [missing]) => {
        ::core::compile_error!("action is a required field for a state");
    };

    (
        queues: $queues:expr,
        $(
            state $name:ident {
                $($body:tt)*
            }
        ),* $(,)?
    ) => {{
        (|| -> Result<$crate::machine::Machine, $crate::errors::ValidationError> {
            // assign sequential indices
            #[expect(clippy::allow_attributes)]
            #[allow(unused_variables)]
            let ($( $name, )*) = {
                let mut _idx = 0usize;
                $(
                    let $name = _idx;
                    _idx += 1;
                )*
                ($( $name, )*)
            };

            let mut states = Vec::new();

            // build states
            $(
                let mut _action = ::core::option::Option::None;
                let mut _probs = ::core::option::Option::None;
                let mut _budget = ::core::option::Option::None;

                // assign state properties, start with [missing] status as action not found
                $crate::machine!(@parse_state _action _probs _budget [missing] $($body)*);

                states.push($crate::state::State::new(
                    _probs,
                    _action.expect("action is a required field for a state"),
                    _budget,
                ));
            )*

            // build the machine
            $crate::machine::Machine::new(states, $queues)
        })()
    }};
}

/// The runtime for a [`Machine`]. Tracks the current machine state, holds it's [`TimedQueue`]s, and
/// any events deferred in [`crate::framework::Framework::process`].
#[derive(Default, Debug, Clone)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    pub(crate) state: usize,

    /// Vector of priority queues corresponding to [`Machine::queues`].
    pub(crate) queues: Vec<TimedQueue<TimedAction>>,

    /// Events deferred to the next [`crate::framework::Framework`] tick.
    pub(crate) deferred_events: Vec<Event>,

    /// Current state budget.
    pub(crate) current_budget: Option<usize>,
}

impl MachineRuntime {
    /// Create a new [`MachineRuntime`] for a given [`Machine`].
    pub fn new<M: Borrow<Machine>>(machine: M) -> Self {
        let m = machine.borrow();
        let queues = m
            .queues
            .iter()
            .map(|capacity| TimedQueue::new(*capacity))
            .collect();
        Self {
            state: 0,
            queues,
            deferred_events: vec![],
            current_budget: m.states.first().and_then(|state| state.decoy_budget),
        }
    }

    /// Pops the action [`TimedQueue`]s.
    pub fn pop_queues(&mut self, now: Instant) -> Box<[(u8, Action)]> {
        let mut actions = Vec::new();

        for (idx, queue) in &mut self.queues.iter_mut().enumerate() {
            // number of queues are always in the u8 range.
            #[expect(clippy::cast_possible_truncation)]
            actions.extend(
                queue
                    .pop_ready(now)
                    .iter()
                    .map(|timed_action| (idx as u8, timed_action.action.clone())),
            );
        }

        actions.into_boxed_slice()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use crate::{
        action::IntegratorAction, distr::Constant, event::Event, framework::Framework,
        state::TransitionProbs,
    };

    use super::*;

    #[test]
    fn test_queues_correct_len() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, [(1, 0.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy, None),
                State::new(None, IntegratorAction::SendDecoy, None),
            ],
            [None; 42],
        )
        .unwrap();
        let framework = Framework::new(machine, rand::rng());

        assert_eq!(
            framework.runtime.queues.len(),
            framework.machine.queues.len(),
        );
    }

    #[test]
    fn test_pop_queues_with_data() {
        let machine = Machine::new(vec![], [None]).unwrap();
        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        let _ = framework.runtime.queues[0].push(TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: now,
        });

        let actions = framework.process(&[], now);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], IntegratorAction::SendDecoy);
    }

    #[test]
    fn test_validate_invalid_state() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, [(3, 0.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy, None),
                State::new(None, IntegratorAction::SendDecoy, None),
            ],
            [None; 42],
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
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), FrameworkAction::CancelQueue(1), None),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 1,
                        delay: Rc::new(Constant(Duration::from_secs(1))),
                    },
                    None,
                ),
            ],
            [None],
        );

        assert!(machine.is_ok());
    }

    #[test]
    fn test_validate_invalid_state_action_queue() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), FrameworkAction::CancelQueue(2), None),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 3,
                        delay: Rc::new(Constant(Duration::from_secs(1))),
                    },
                    None,
                ),
            ],
            [None],
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
            TransitionProbs::new([(Event::SendNormal, [(5, 1.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), FrameworkAction::CancelQueue(2), None),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 3,
                        delay: Rc::new(Constant(Duration::from_secs(1))),
                    },
                    None,
                ),
            ],
            [None],
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
