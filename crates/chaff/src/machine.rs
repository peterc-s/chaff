//! Chaff machines

use crate::state::State;

/// The Chaff machine specification.
#[derive(Default, Debug, Clone)]
pub struct Machine {
    pub(crate) states: Vec<State>,
}

impl Machine {
    /// Create a new [`Machine`] with the given states.
    pub fn new(states: Vec<State>) -> Self {
        Self { states }
    }
}

/// The runtime for a machine.
#[derive(Default, Debug, Clone, Copy)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    pub(crate) state: usize,
}
