//! State and transition representations for [`crate::machine::Machine`]s.

use std::collections::HashMap;

use rand::Rng;

use crate::{action::Action, errors::ValidationError, event::Event};

/// Represents a single state in a [`crate::machine::Machine`].
#[derive(Debug, Clone)]
pub struct State {
    /// The probabilities of transitioning to other states in the machine.
    pub(crate) trans_probs: Option<TransitionProbs>,

    /// The action to take on transitioning to this state.
    pub(crate) action: Action,
}

impl State {
    /// Create a new state with the given [`TransitionProbs`] and [`Action`] to take on transition.
    pub fn new(trans_probs: impl Into<Option<TransitionProbs>>, action: impl Into<Action>) -> Self {
        Self {
            trans_probs: trans_probs.into(),
            action: action.into(),
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
pub struct TransitionProbs(pub HashMap<Event, Transition>);

impl TransitionProbs {
    /// Construct a new [`TransitionProbs`] using the given [`Event`] to [`Transition`] mapping pairs.
    ///
    /// Trying to map the same [`Event`]
    ///
    /// # Example
    /// ```rust
    /// use chaff::{event::Event, state::{Transition, TransitionProbs}};
    ///
    /// let trans_probs = TransitionProbs::new([
    ///     (Event::SendNormal, (1, 0.5).try_into().unwrap()),
    ///     (Event::QueuePopped(1), (2, 0.5).try_into().unwrap()),
    /// ]).unwrap();
    /// ```
    pub fn new(
        pairs: impl IntoIterator<Item = (Event, Transition)>,
    ) -> Result<Self, ValidationError> {
        let map: HashMap<Event, Transition> = pairs.into_iter().collect();
        let sum = map.values().map(|t| t.prob).sum::<f32>();
        if (0.0..=1.0).contains(&sum) {
            Ok(Self(map))
        } else {
            Err(ValidationError::BadTransitionProbs(sum))
        }
    }

    /// Try to get the [`Transition`] associated with the given [`Event`].
    pub fn get(&self, event: Event) -> Option<Transition> {
        self.0.get(&event).copied()
    }

    /// "Trigger" a transition probabilistically based on the given event. Returns [`None`] if no
    /// transition occurs.
    pub fn trigger(&self, rng: &mut impl Rng, event: Event) -> Option<usize> {
        self.get(event).and_then(|trans| {
            if trans.prob >= rng.random() {
                Some(trans.index)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_probs_validation() {
        let over_1 = TransitionProbs::new([
            (Event::SendNormal, (0, 1.0).try_into().unwrap()),
            (Event::ReceiveNormal, (0, f32::EPSILON).try_into().unwrap()),
        ]);

        assert!(over_1.is_err());

        let exact_1 = TransitionProbs::new([
            (Event::SendNormal, (0, 1.0).try_into().unwrap()),
            (Event::ReceiveNormal, (0, 0.0).try_into().unwrap()),
        ]);

        assert!(exact_1.is_ok());

        let negative: Result<Transition, _> = (0, -f32::EPSILON).try_into();
        match negative {
            #[expect(clippy::float_cmp)]
            Err(ValidationError::BadTransitionProbs(prob)) => {
                assert_eq!(prob, -f32::EPSILON);
            }
            other => panic!("unexpected result: {other:?}"),
        }

        let over_1: Result<Transition, _> = (0, 1.0 + f32::EPSILON).try_into();
        match over_1 {
            #[expect(clippy::float_cmp)]
            Err(ValidationError::BadTransitionProbs(prob)) => {
                assert_eq!(prob, 1.0 + f32::EPSILON);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
