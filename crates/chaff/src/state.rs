//! State and transition representations for [`crate::machine::Machine`]s.

use std::collections::HashMap;

use rand::RngExt;

use crate::{action::Action, errors::ValidationError, event::Event};

/// Represents a single state in a [`crate::machine::Machine`].
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// The probabilities of transitioning to other states in the machine.
    pub(crate) trans_probs: Option<TransitionProbs>,

    /// The action to take on transitioning to this state.
    pub(crate) action: Option<Action>,

    /// The number of decoys this state can send via self-transition (includes initial transition to
    /// this state).
    pub(crate) decoy_budget: Option<usize>,
}

impl State {
    /// Create a new state with the given [`TransitionProbs`], [`Action`] to take on transition, and
    /// a budget for the number of decoys the state can send during self-transition.
    pub fn new(
        trans_probs: impl Into<Option<TransitionProbs>>,
        action: Option<impl Into<Action>>,
        decoy_budget: Option<usize>,
    ) -> Self {
        Self {
            trans_probs: trans_probs.into(),
            action: action.map(std::convert::Into::into),
            decoy_budget,
        }
    }
}

/// Represents a stochastic transition from a state to another at [`Transition::index`] with
/// probability [`Transition::prob`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// The index of the state to transition to.
    pub index: usize,

    /// The probability of transitioning to that state.
    pub prob: f32,
}

impl TryFrom<(usize, f32)> for Transition {
    type Error = ValidationError;

    fn try_from(value: (usize, f32)) -> Result<Self, Self::Error> {
        if (0.0..=1.0).contains(&value.1) {
            Ok(Self {
                index: value.0,
                prob: value.1,
            })
        } else {
            Err(ValidationError::BadTransitionProbs(value.1))
        }
    }
}

/// Represents the transition probabilities for all events.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionProbs(pub HashMap<Event, Vec<Transition>>);

impl TransitionProbs {
    /// Construct a new [`TransitionProbs`] using the given [`Event`] to [`Vec<Transition>`] mapping pairs.
    ///
    /// # Caveat
    ///
    /// Trying to map the same [`Event`] multiple times will result in only the last mapping being
    /// applied.
    ///
    /// # Errors
    ///
    /// Can give a [`ValidationError::BadTransitionProbs`] if the given transition probabilities sum
    /// up to a value outside of the `0.0..=1.0` range.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chaff::{event::Event, state::{Transition, TransitionProbs}};
    ///
    /// let trans_probs = TransitionProbs::new([
    ///     (Event::SendNormal, [(1, 0.5).try_into().unwrap()]),
    ///     (Event::QueuePopped(1), [(2, 0.5).try_into().unwrap()]),
    /// ]).unwrap();
    /// ```
    pub fn new(
        pairs: impl IntoIterator<Item = (Event, impl Into<Vec<Transition>>)>,
    ) -> Result<Self, ValidationError> {
        let mut map: HashMap<Event, Vec<Transition>> = HashMap::new();

        for (event, transitions) in pairs {
            map.entry(event).or_default().extend(transitions.into());
        }

        for transitions in map.values() {
            let sum: f32 = transitions.iter().map(|t| t.prob).sum();
            if !(0.0..=1.0).contains(&sum) {
                return Err(ValidationError::BadTransitionProbs(sum));
            }
        }

        Ok(Self(map))
    }

    /// Construct a new [`TransitionProbs`] using an array of tuples.
    ///
    /// The same caveat in [`TransitionProbs::new`] applies here.
    ///
    /// # Errors
    ///
    /// If an individual transition probability or sum of transition probabilities fall outside of
    /// the `0.0..=1.0` range, will throw a [`ValidationError::BadTransitionProbs`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use chaff::{event::Event, state::{Transition, TransitionProbs}};
    ///
    /// let trans_probs = TransitionProbs::from_tuples([
    ///     (Event::SendNormal, [(1, 0.5)]),
    ///     (Event::QueuePopped(1), [(2, 0.5)]),
    /// ]).unwrap();
    /// ```
    pub fn from_tuples<const N: usize>(
        transitions: [(Event, impl Into<Vec<(usize, f32)>>); N],
    ) -> Result<Self, ValidationError> {
        let mut valid_transitions = vec![];
        let mut validation_errors = vec![];

        for (event, target_tuples) in transitions {
            let tuples: Vec<(usize, f32)> = target_tuples.into();
            let mut event_transitions = vec![];

            for tuple in tuples {
                match Transition::try_from(tuple) {
                    Ok(transition) => event_transitions.push(transition),
                    Err(err) => validation_errors.push(err),
                }
            }

            valid_transitions.push((event, event_transitions));
        }

        if !validation_errors.is_empty() {
            return Err(ValidationError::Multiple(
                validation_errors.into_boxed_slice(),
            ));
        }

        Self::new(valid_transitions)
    }

    /// Try to get the [`Vec<Transition>`] associated with the given [`Event`].
    #[must_use]
    pub fn get(&self, event: Event) -> Option<&Vec<Transition>> {
        self.0.get(&event)
    }

    /// "Trigger" a transition probabilistically based on the given event. Returns [`None`] if no
    /// transition occurs.
    pub fn trigger(&self, rng: &mut impl RngExt, event: Event) -> Option<usize> {
        let transitions = self.0.get(&event)?;
        let mut roll: f32 = rng.random();

        for transition in transitions {
            if roll < transition.prob {
                return Some(transition.index);
            }
            roll -= transition.prob;
        }

        None
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_probs_validation() {
        let over_1 = TransitionProbs::new([(
            Event::SendNormal,
            [(0, 0.6).try_into().unwrap(), (1, 0.5).try_into().unwrap()],
        )]);
        assert!(over_1.is_err());

        let exact_1 = TransitionProbs::new([(
            Event::SendNormal,
            [(0, 0.5).try_into().unwrap(), (1, 0.5).try_into().unwrap()],
        )]);
        assert!(exact_1.is_ok());

        let multiple_events_exact_1 = TransitionProbs::new([
            (Event::SendNormal, [(0, 1.0).try_into().unwrap()]),
            (Event::ReceiveNormal, [(0, 1.0).try_into().unwrap()]),
        ]);
        assert!(multiple_events_exact_1.is_ok());

        let negative: Result<Transition, _> = (0, -f32::EPSILON).try_into();
        match negative {
            #[expect(clippy::float_cmp)]
            Err(ValidationError::BadTransitionProbs(prob)) => {
                assert_eq!(prob, -f32::EPSILON);
            }
            other => panic!("unexpected result: {other:?}"),
        }

        let over_1_single: Result<Transition, _> = (0, 1.0 + f32::EPSILON).try_into();
        match over_1_single {
            #[expect(clippy::float_cmp)]
            Err(ValidationError::BadTransitionProbs(prob)) => {
                assert_eq!(prob, 1.0 + f32::EPSILON);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_invalid_transition_probs_from_tuples() {
        assert!(TransitionProbs::from_tuples([(Event::SendNormal, [(1, 1.5)])]).is_err());
        assert!(TransitionProbs::from_tuples([(Event::SendNormal, [(1, -0.1)])]).is_err());
        assert!(TransitionProbs::from_tuples([(Event::SendNormal, [(1, f32::NAN)])]).is_err());
        assert!(TransitionProbs::from_tuples([(Event::SendNormal, [(1, 0.6), (2, 0.5)])]).is_err());
    }

    #[test]
    fn test_get_transition() {
        let trans_probs = TransitionProbs::new([(
            Event::SendNormal,
            [(1, 0.5).try_into().unwrap(), (2, 0.5).try_into().unwrap()],
        )])
        .unwrap();

        let send_transitions = trans_probs.get(Event::SendNormal);
        assert!(send_transitions.is_some());
        assert_eq!(send_transitions.unwrap().len(), 2);

        let receive_transitions = trans_probs.get(Event::ReceiveNormal);
        assert!(receive_transitions.is_none());
    }

    #[test]
    fn test_probs_validation_multiple_same_event() {
        let over_1_combined = TransitionProbs::new([
            (Event::SendNormal, [(0, 0.6).try_into().unwrap()]),
            (Event::SendNormal, [(1, 0.5).try_into().unwrap()]),
        ]);

        #[expect(clippy::float_cmp)]
        match over_1_combined {
            Err(ValidationError::BadTransitionProbs(prob)) => {
                assert_eq!(prob, 1.1);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
