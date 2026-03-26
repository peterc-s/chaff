//! State and transition representations for [`crate::machine::Machine`]s.

use std::ops::{Index, IndexMut};

use rand::Rng;

use crate::{action::Action, errors::ValidationError, event::Event};

/// Represents a single state in a [`crate::machine::Machine`].
#[derive(Default, Debug, Clone, Copy)]
pub struct State {
    /// The probabilities of transitioning to other states in the machine.
    pub(crate) trans_probs: Option<TransitionProbs>,

    /// The action to take on transitioning to this state.
    pub(crate) action: Action,
}

impl State {
    /// Create a new state with the given [`TransitionProbs`] and [`Action`] to take on transition.
    pub fn new(trans_probs: Option<TransitionProbs>, action: Action) -> Self {
        Self {
            trans_probs,
            action,
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionProbs(pub [Option<Transition>; Event::COUNT]);

impl TransitionProbs {
    fn empty() -> Self {
        Self([const { None }; Event::COUNT])
    }

    /// Create transition probabilities using a function.
    ///
    /// # Example
    /// ```rust
    /// use chaff::{event::Event, state::{Transition, TransitionProbs}};
    ///
    /// let trans_probs = TransitionProbs::from_fn(|event| match event {
    ///     Event::SendNormal => Some((1, 0.5).try_into().unwrap()), // transition to state 1 with probability 0.5
    ///     Event::ReceiveNormal => None,
    /// }).unwrap();
    ///
    /// let trans = trans_probs[Event::SendNormal].unwrap();
    /// assert_eq!(trans, Transition { index: 1, prob: 0.5 });
    /// ```
    pub fn from_fn<F>(mut f: F) -> Result<Self, ValidationError>
    where
        F: FnMut(Event) -> Option<Transition>,
    {
        let mut probs = Self::empty();
        for event in Event::ALL {
            probs[event] = f(event);
        }

        // get sum of transition probabilities
        let sum = probs.0.iter().flatten().map(|x| x.prob).sum::<f32>();

        if (0.0..=1.0).contains(&sum) {
            Ok(probs)
        } else {
            Err(ValidationError::BadTransitionProbs(sum))
        }
    }

    /// "Trigger" a transition probabilistically based on the given event. Returns [`None`] if no
    /// transition occurs.
    pub fn trigger(&self, rng: &mut impl Rng, event: Event) -> Option<usize> {
        self[event].and_then(|trans| {
            if trans.prob >= rng.random() {
                Some(trans.index)
            } else {
                None
            }
        })
    }
}

impl Index<Event> for TransitionProbs {
    type Output = Option<Transition>;

    fn index(&self, index: Event) -> &Self::Output {
        &self.0[index.index()]
    }
}

impl IndexMut<Event> for TransitionProbs {
    fn index_mut(&mut self, index: Event) -> &mut Self::Output {
        &mut self.0[index.index()]
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_probs_validation() {
        let over_1 = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((0, 1.0).try_into().unwrap()),
            Event::ReceiveNormal => Some((0, f32::EPSILON).try_into().unwrap()),
        });

        assert!(over_1.is_err());

        let exact_1 = TransitionProbs::from_fn(|event| match event {
            Event::SendNormal => Some((0, 1.0).try_into().unwrap()),
            Event::ReceiveNormal => Some((0, 0.0).try_into().unwrap()),
        });

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
