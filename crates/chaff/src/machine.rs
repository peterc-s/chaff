//! Chaff machines

use std::{borrow::Borrow, time::Instant};

use crate::{
    action::Action,
    queue::{TimedAction, TimedQueue},
    state::State,
};

/// The Chaff machine specification.
#[derive(Default, Debug, Clone)]
pub struct Machine {
    pub(crate) states: Vec<State>,
    pub(crate) queues: u8,
}

impl Machine {
    /// Create a new [`Machine`] with the given states.
    pub fn new(states: Vec<State>, queues: u8) -> Self {
        Self { states, queues }
    }
}

/// The runtime for a machine.
#[derive(Default, Debug, Clone)]
pub struct MachineRuntime {
    /// Index into the [`Machine::states`] array.
    pub(crate) state: usize,

    /// Vector of priority queues corresponding to [`Machine::queues`].
    pub(crate) queues: Vec<TimedQueue<TimedAction>>,
}

impl MachineRuntime {
    /// Create a new [`MachineRuntime`] for a given [`Machine`].
    pub fn new<M: Borrow<Machine>>(machine: M) -> Self {
        let m = machine.borrow();
        let queues = (0..m.queues).map(|_| TimedQueue::new()).collect();
        Self { state: 0, queues }
    }

    /// Pops the action [`TimedQueues`].
    pub fn pop_queues(&mut self, now: Instant) -> Box<[Action]> {
        let mut actions = Vec::new();

        for queue in &mut self.queues {
            actions.extend(queue.pop_ready(now));
        }

        actions
            .iter()
            .map(|timed_action| timed_action.action)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}
