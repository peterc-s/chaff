use std::ops::{Index, IndexMut};

use rand::Rng;

use crate::{action::Action, event::Event};

#[derive(Default, Debug, Clone, Copy)]
pub struct State {
    pub(crate) trans_probs: Option<TransitionProbs>,
    pub(crate) action: Action,
}

impl State {
    pub fn new(trans_probs: Option<TransitionProbs>, action: Action) -> Self {
        Self {
            trans_probs,
            action,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub index: usize, // Index of state to transition to.
    pub prob: f32,    // Probability of transition.
}

impl From<(usize, f32)> for Transition {
    fn from(value: (usize, f32)) -> Self {
        Self {
            index: value.0,
            prob: value.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionProbs(pub [Option<Transition>; Event::COUNT]);

impl TransitionProbs {
    fn empty() -> Self {
        Self([const { None }; Event::COUNT])
    }

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

    pub fn trigger(&self, rng: &mut impl Rng, event: Event) -> Option<usize> {
        self[event].and_then(|trans| {
            if trans.prob < rng.random() {
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
