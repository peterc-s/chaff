//! The Chaff framework.
// TODO: document

#![expect(dead_code)]
#![expect(missing_docs)]

use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct Framework {
    machine: Machine,
    runtime: MachineRuntime,
}

impl Framework {
    pub fn new(machine: Machine) -> Self {
        Self {
            machine,
            runtime: MachineRuntime::default(),
        }
    }

    pub fn trigger_events(&self, _events: &[Event]) -> Box<[Action]> {
        todo!()
    }
}

#[derive(Default)]
pub struct Machine {
    states: Vec<State>,
}

impl Machine {
    pub fn new(states: Vec<State>) -> Self {
        Self { states }
    }
}

#[derive(Default)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    state: usize,
}

#[derive(Default)]
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
pub struct Transition(
    pub usize, // Index of state to transition to.
    pub f32,   // Probability of transition.
);

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

#[derive(Default)]
pub enum Action {
    #[default]
    SendDecoy,
}

// For easily working with events.
macro_rules! enum_index {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[repr(usize)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
