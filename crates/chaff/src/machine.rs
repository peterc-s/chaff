//! The Chaff [`Machine`].

use std::{borrow::Borrow, time::Instant};

use crate::{
    action::{Action, FrameworkAction},
    errors::ValidationError,
    event::Event,
    queue::{TimedAction, TimedQueue},
    state::State,
};

/// Represents a [`Machine`]s decoy budget.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MachineDecoyBudget {
    /// Machine can only send up to this many decoy packets before getting limited.
    Absolute(usize),

    /// Machine can only send this proportion of real traffic as decoys before getting limited.
    Proportion(f64),
}

/// The Chaff machine specification. Represents a queue automata with [`State`]s and
/// [`TimedQueue`]s.
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub(crate) states: Vec<State>,
    pub(crate) queues: Vec<Option<usize>>,
    pub(crate) budget: Option<MachineDecoyBudget>,
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
    fn validate(
        states: &[State],
        queues: usize,
        budget: Option<MachineDecoyBudget>,
    ) -> Result<(), ValidationError> {
        let Ok(queues) = u8::try_from(queues) else {
            return Err(ValidationError::TooManyQueues(queues));
        };

        let mut errors = vec![];
        let num_states = states.len();

        if num_states == 0 {
            return Err(ValidationError::NoStates);
        }

        // check for transitions to invalid states
        let mut invalid_transitions = states
            .iter()
            .filter_map(|state| state.trans_probs.as_ref())
            .flat_map(|probs| probs.0.values())
            .flat_map(|transitions| transitions.iter().map(|transition| transition.index))
            .filter(|&index| index > num_states)
            .peekable();

        if invalid_transitions.peek().is_some() {
            errors.push(ValidationError::TransitionToInvalidState(
                invalid_transitions.collect(),
            ));
        }

        // check for state actions which would try to interact with
        // non-existent queues
        let mut invalid_state_action_queues = states
            .iter()
            .map(|state| state.actions.clone())
            .flat_map(|actions| {
                actions.into_iter().map(|action| match action {
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
            })
            .flatten()
            .peekable();

        if invalid_state_action_queues.peek().is_some() {
            errors.push(ValidationError::InvalidStateActionQueue(
                invalid_state_action_queues.collect(),
            ));
        }

        // don't allow negative percent decoy budgets
        if let Some(MachineDecoyBudget::Proportion(percent)) = budget
            && percent < 0.0
        {
            errors.push(ValidationError::NegativeProportion(percent));
        }

        // TODO: maybe validate transitions too, i.e. no point in having a QueueFilled(10)
        // transition when there's only 10 queues.

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
    pub fn try_new(
        states: impl Into<Vec<State>>,
        queues: impl Into<Vec<Option<usize>>>,
        budget: Option<MachineDecoyBudget>,
    ) -> Result<Self, ValidationError> {
        let queues = queues.into();
        let states = states.into();
        Self::validate(&states, queues.len(), budget)?;
        Ok(Self {
            states,
            queues,
            budget,
        })
    }
}

#[cfg(feature = "borsh")]
impl borsh::BorshDeserialize for Machine {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let states: Vec<State> = borsh::BorshDeserialize::deserialize_reader(reader)?;
        let queues: Vec<Option<usize>> = borsh::BorshDeserialize::deserialize_reader(reader)?;
        let budget: Option<MachineDecoyBudget> =
            borsh::BorshDeserialize::deserialize_reader(reader)?;

        Self::validate(&states, queues.len(), budget).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{err:?}"))
        })?;

        Ok(Self {
            states,
            queues,
            budget,
        })
    }
}

/// Create a machine with named states.
///
/// # Example
///
/// ```rust
/// use chaff::{
///     machine,
///     action::{Action, IntegratorAction},
///     event::Event,
///     machine::{Machine, MachineDecoyBudget},
///     state::{State, TransitionProbs}
/// };
///
/// let machine_macro = machine! {
///     queues: [],
///     budget: Proportion(0.5),
///     
///     state init {
///         actions: [IntegratorAction::SendDecoy],
///         budget: 25,
///         transitions: [
///             Event::SendNormal => [(jump, 0.5)],
///             Event::ReceiveNormal => jump
///         ],
///     },
///
///     state jump {
///         transitions: [
///             Event::SendNormal => end,
///         ]
///     },
///     
///     state end {
///         actions: [IntegratorAction::SendDecoy],
///     }
/// }.unwrap();
///
/// let machine_manual = Machine::try_new(
///     vec![
///         State::new(
///             Some(TransitionProbs::from_tuples([
///                 (Event::SendNormal, [(1, 0.5)]),
///                 (Event::ReceiveNormal, [(1, 1.0)])
///             ]).unwrap()),
///             Some(IntegratorAction::SendDecoy),
///             Some(25),
///         ),
///         State::new(
///             Some(TransitionProbs::from_tuples([
///                 (Event::SendNormal, [(2, 1.0)])
///             ]).unwrap()),
///             None::<Action>,
///             None,
///         ),
///         State::new(None, Some(IntegratorAction::SendDecoy), None)
///     ],
///     [],
///     Some(MachineDecoyBudget::Proportion(0.5)),
/// ).unwrap();
///
/// assert_eq!(machine_macro, machine_manual);
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
    (@parse_state $a:ident $t:ident $b:ident actions: [ $( $action:expr ),* $(,)? ] $(, $($rest:tt)*)? ) => {
        $(
            $a.push($action.into());
        )*
        $crate::machine!(@parse_state $a $t $b $($($rest)*)?);
    };

    // parse `transitions:`
    (@parse_state $a:ident $t:ident $b:ident transitions: [ $( $event:expr => $targets:tt ),* $(,)? ] $(, $($rest:tt)*)? ) => {
        $t = ::core::option::Option::Some(
            $crate::state::TransitionProbs::from_tuples([
                $( ($event, $crate::machine!(@targets $targets)) ),*
            ])?
        );
        $crate::machine!(@parse_state $a $t $b $($($rest)*)?);
    };

    // parse `budget:`
    (@parse_state $a:ident $t:ident $b:ident budget: $budget:expr $(, $($rest:tt)*)? ) => {
        $b = ::core::option::Option::Some($budget);
        $crate::machine!(@parse_state $a $t $b $($($rest)*)?);
    };

    // base case
    (@parse_state $a:ident $t:ident $b:ident) => {};

    (
        @build
        queues: $queues:expr,
        budget: $budget:expr,
        $(
            state $name:ident {
                $($body:tt)*
            }
        ),* $(,)?
    ) => {{
        // some invocations may cause a redundant closure call, so we allow it.
        // workspace lints don't allow `allow` attributes, so expect a violation of it.
        // aren't attributes great?
        #[expect(clippy::allow_attributes)]
        #[allow(clippy::redundant_closure_call)]
        (|| -> Result<$crate::machine::Machine, $crate::errors::ValidationError> {
            // assign sequential indices
            #[expect(clippy::allow_attributes)]
            #[allow(unused_variables)]
            // this only happens when there are no states, which is already a validation error
            #[allow(clippy::unused_unit)]
            let ($( $name, )*) = {
                let mut _idx = 0usize;
                $(
                    let $name = _idx;
                    _idx += 1;
                )*
                ($( $name, )*)
            };

            // this only happens when there are no states, which is already a validation error
            #[expect(clippy::allow_attributes)]
            #[allow(unused_mut)]
            let mut states = Vec::new();

            // build states
            $(
                let mut _actions: std::vec::Vec<$crate::action::Action> = ::std::vec::Vec::new();
                let mut _probs = ::core::option::Option::None;
                let mut _budget = ::core::option::Option::None;

                // assign state properties
                $crate::machine!(@parse_state _actions _probs _budget $($body)*);

                states.push($crate::state::State::new(
                    _probs,
                    _actions.into_boxed_slice(),
                    _budget,
                ));
            )*

            // build the machine
            $crate::machine::Machine::try_new(states, $queues, $budget)
        })()
    }};

    (
        queues: $queues:expr,
        budget: $variant:ident ($($args:tt)*),
        $( state $name:ident { $($body:tt)* } ),* $(,)?
    ) => {
        $crate::machine!(
            @build
            queues: $queues,
            budget: ::core::option::Option::Some(
                $crate::machine::MachineDecoyBudget::$variant($($args)*)
            ),
            $( state $name { $($body)* } ),*
        )
    };

    (
        queues: $queues:expr,
        $( state $name:ident { $($body:tt)* } ),* $(,)?
    ) => {
        $crate::machine!(
            @build
            queues: $queues,
            budget: ::core::option::Option::None,
            $( state $name { $($body)* } ),*
        )
    };
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
    pub(crate) current_state_budget: Option<usize>,

    /// If the machine has been used at all yet. Used for checking if the initial state's action has
    /// been processed or sent to the integrator.
    pub(crate) initialised: bool,

    /// Total decoy packets sent.
    pub(crate) decoys_sent: usize,

    /// Total real packets sent.
    pub(crate) real_sent: usize,

    /// If the budget is proportional and has currently been reached.
    pub(crate) proportion_blocked: bool,
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
            current_state_budget: m.states.first().and_then(|state| state.decoy_budget),
            initialised: false,
            decoys_sent: 0,
            real_sent: 0,
            proportion_blocked: false,
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

    /// Peeks the soonest [`Instant`] in the [`TimedQueue`]s.
    pub fn peek_soonest_scheduled_instant(&self) -> Option<Instant> {
        self.queues
            .iter()
            .filter_map(TimedQueue::peek_soonest_instant)
            .min()
    }

    /// Returns the number of decoy packets sent.
    #[must_use]
    pub fn get_decoys_sent(&self) -> usize {
        self.decoys_sent
    }

    /// Returns the number of real packets sent.
    #[must_use]
    pub fn get_real_sent(&self) -> usize {
        self.real_sent
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::expect_used)]
mod tests {
    use crate::{
        action::IntegratorAction,
        distr::{Distr, DistrKind},
        event::Event,
        framework::Framework,
        state::TransitionProbs,
    };

    use super::*;

    #[test]
    fn test_queues_correct_len() {
        let trans_probs =
            TransitionProbs::try_new([(Event::SendNormal, [(1, 0.0).try_into().unwrap()])])
                .unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), [IntegratorAction::SendDecoy], None),
                State::new(None, [IntegratorAction::SendDecoy], None),
            ],
            [None; 42],
            None,
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
        let machine = machine! {
            queues: [None],
            state init {}
        }
        .unwrap();
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
            TransitionProbs::try_new([(Event::SendNormal, [(3, 0.0).try_into().unwrap()])])
                .unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), [IntegratorAction::SendDecoy], None),
                State::new(None, [IntegratorAction::SendDecoy], None),
            ],
            [None; 42],
            None,
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
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let const_distr: Distr = DistrKind::Constant(1.0).try_into().unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), [FrameworkAction::CancelQueue(1)], None),
                State::new(
                    None,
                    [FrameworkAction::schedule(
                        IntegratorAction::SendDecoy,
                        1,
                        const_distr,
                    )],
                    None,
                ),
            ],
            [None],
            None,
        );

        assert!(machine.is_ok());
    }

    #[test]
    fn test_validate_invalid_state_action_queue() {
        let trans_probs =
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let const_distr: Distr = DistrKind::Constant(1.0).try_into().unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), [FrameworkAction::CancelQueue(2)], None),
                State::new(
                    None,
                    [FrameworkAction::schedule(
                        IntegratorAction::SendDecoy,
                        3,
                        const_distr,
                    )],
                    None,
                ),
            ],
            [None],
            None,
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
            TransitionProbs::try_new([(Event::SendNormal, [(5, 1.0).try_into().unwrap()])])
                .unwrap();

        let const_distr: Distr = DistrKind::Constant(1.0).try_into().unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), [FrameworkAction::CancelQueue(2)], None),
                State::new(
                    None,
                    [FrameworkAction::schedule(
                        IntegratorAction::SendDecoy,
                        3,
                        const_distr,
                    )],
                    None,
                ),
            ],
            [None],
            None,
        );

        match machine {
            Err(ValidationError::Multiple(errors)) => {
                let num_errors = errors.len();
                assert_eq!(num_errors, 2, "unexpected number of errors: {num_errors}");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_too_many_queues() {
        const U8_MAX_PLUS_ONE: usize = u8::MAX as usize + 1;
        let err = Machine::try_new([], [None; U8_MAX_PLUS_ONE], None);
        assert!(matches!(
            err,
            Err(ValidationError::TooManyQueues(U8_MAX_PLUS_ONE))
        ));
    }

    #[test]
    fn test_no_states_validation() {
        let err = machine! {
            queues: [],
        };

        assert!(matches!(err, Err(ValidationError::NoStates)));
    }

    #[test]
    fn test_negative_proportion_validation() {
        let err = machine! {
            queues: [],
            budget: Proportion(-0.1),
            state init {},
        };

        assert!(matches!(
            err,
            Err(ValidationError::NegativeProportion(-0.1))
        ));
    }

    #[cfg(feature = "borsh")]
    mod borsh {
        use super::*;
        use std::{
            fs::File,
            io::{Read as _, Seek as _},
            path::PathBuf,
        };

        use ::borsh::{BorshDeserialize as _, BorshSerialize as _};
        use tempfile::NamedTempFile;

        #[test]
        fn test_borsh_machine_round_trip() {
            let machine = machine! {
                queues: [Some(4), None],
                budget: Proportion(0.67),

                state init {
                    actions: [IntegratorAction::SendDecoy],
                    budget: 25,
                    transitions: [
                        Event::SendNormal => [(jump, 0.5), (end, 0.5)],
                        Event::ReceiveNormal => jump
                    ],
                },

                state jump {
                    transitions: [
                        Event::SendNormal => end,
                        Event::ReceiveNormal => other,
                    ]
                },

                state other {
                    actions: [FrameworkAction::schedule(
                        IntegratorAction::SendDecoy,
                        0,
                        DistrKind::Uniform {
                            low: 0.1,
                            high: 0.2
                        }.try_into().unwrap()
                    )],
                    transitions: [
                        Event::SendNormal => end,
                    ]
                },

                state end {
                    actions: [IntegratorAction::SendDecoy],
                }
            }
            .unwrap();

            let mut file = NamedTempFile::new().unwrap();
            // let mut tmp = std::env::temp_dir();
            // tmp.push("test-extra.machine");
            // let mut tmp_file = File::create(tmp).unwrap();
            // machine.serialize(&mut tmp_file).unwrap();

            machine.serialize(&mut file).expect("failed to serialize");
            file.rewind().unwrap();

            let mut bytes = vec![];
            file.read_to_end(&mut bytes).expect("Failed to read");
            let machine_de = Machine::deserialize(&mut bytes.as_slice()).unwrap();

            assert_eq!(machine, machine_de);
        }

        #[test]
        fn test_borsh_machine_nan_probs() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/nan-probability.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }

        #[test]
        fn test_borsh_machine_bad_probs() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/corrupt-probability.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }

        #[test]
        fn test_borsh_machine_bad_states_len() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/corrupt-states-len.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }

        #[test]
        fn test_borsh_machine_bad_queue_access() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/corrupt-queue-access.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }

        #[test]
        fn test_borsh_machine_bad_trans_probs_sum_probability() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/corrupt-sum-probability.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }

        #[test]
        fn test_borsh_machine_bad_distr_params() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/corrupt-distr.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }

        #[test]
        fn test_borsh_machine_negative_proportion() {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("test-machines/negative-proportion-budget.machine");

            let mut file = File::open(path).unwrap();
            let err = Machine::deserialize_reader(&mut file);
            match err {
                Err(ref err) if err.kind() == std::io::ErrorKind::InvalidData => {}
                Ok(other) => panic!("unexpected result: {other:?}"),
                Err(other) => panic!("unexpected result: {other:?}"),
            }
        }
    }
}
