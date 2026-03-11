use crate::state::State;

#[derive(Default, Debug, Clone)]
pub struct Machine {
    pub(crate) states: Vec<State>,
}

impl Machine {
    pub fn new(states: Vec<State>) -> Self {
        Self { states }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    pub(crate) state: usize,
}
