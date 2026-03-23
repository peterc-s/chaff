//! State and transition representations for [`crate::machine::Machine`]s.

use std::ops::{Index, IndexMut};

use rand::Rng;

use crate::{action::Action, event::Event};

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

impl From<(usize, f32)> for Transition {
    fn from(value: (usize, f32)) -> Self {
        Self {
            index: value.0,
            prob: value.1,
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
    ///     Event::SendNormal => Some((1, 0.5).into()), // transition to state 1 with probability 0.5
    ///     Event::ReceiveNormal => None,
    /// });
    ///
    /// let trans = trans_probs[Event::SendNormal].unwrap();
    /// assert_eq!(trans, Transition { index: 1, prob: 0.5 });
    /// ```
    pub fn from_fn<F>(mut f: F) -> Self
    where
        F: FnMut(Event) -> Option<Transition>,
    {
        let mut probs = Self::empty();
        for event in Event::ALL {
            probs[event] = f(event);
        }
        probs
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
