//! The Chaff framework.
// TODO: document

#![expect(missing_docs)]

use std::ops::{Index, IndexMut};

use rand::Rng;

#[derive(Default, Debug, Clone)]
pub struct Framework<R: Rng> {
    machine: Machine,
    runtime: MachineRuntime,
    rng: R,
}

impl<R: Rng> Framework<R> {
    pub fn new(machine: Machine, rng: R) -> Self {
        Self {
            machine,
            runtime: MachineRuntime::default(),
            rng,
        }
    }

    fn get_trans_probs(&self) -> Option<TransitionProbs> {
        self.machine
            .states
            .get(self.runtime.state)
            .map(|state| state.trans_probs)?
    }

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

    pub fn get_state(&self) -> usize {
        self.runtime.state
    }
}

#[derive(Default, Debug, Clone)]
pub struct Machine {
    states: Vec<State>,
}

impl Machine {
    pub fn new(states: Vec<State>) -> Self {
        Self { states }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    state: usize,
}

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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    #[default]
    SendDecoy,
}

// For easily working with events.
macro_rules! enum_index {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[repr(usize)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const COUNT: usize = enum_index!(@count $($variant),+);

            pub const ALL: [Self; Self::COUNT] = [
                $(Self::$variant),+
            ];

            pub const fn index(self) -> usize {
                self as usize
            }
        }
    };

    // token counting dark arts...
    (@count $($tts:tt),*) => {
        // this is using the array of units len method
        // to count the "replace" of each token, which, from
        // the name rule is the variants of the enum
        // the replace rule simply replaces each variant with a unit
        // therefore, if we have two variants, this should become
        // <[()]>::len(&[(), ()]) which is 2, as it should be.
        <[()]>::len(&[$(enum_index!(@replace $tts ())),*])
    };

    (@replace $_t:tt $sub:expr) => {$sub};
}

enum_index! {
    Event {
        SendNormal,
        ReceiveNormal,
    }
}
