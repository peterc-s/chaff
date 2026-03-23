#![expect(missing_docs)]

use std::{collections::BinaryHeap, time::Instant};

use crate::action::Action;

pub trait Timed {
    fn ready(&self, now: Instant) -> bool;
    fn execute_at(&self) -> Instant;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Scheduled<T: Timed>(pub T);

impl<T: Timed> PartialEq for Scheduled<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.execute_at() == other.0.execute_at()
    }
}

impl<T: Timed> Eq for Scheduled<T> {}

impl<T: Timed> PartialOrd for Scheduled<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Timed> Ord for Scheduled<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // flipped order for min-heap
        self.0.execute_at().cmp(&other.0.execute_at()).reverse()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimedAction {
    pub execute_at: Instant,
    pub action: Action,
}

impl Timed for TimedAction {
    fn ready(&self, now: Instant) -> bool {
        self.execute_at <= now
    }

    fn execute_at(&self) -> Instant {
        self.execute_at
    }
}

#[derive(Debug, Clone, Default)]
pub struct TimedQueue<T: Timed> {
    queue: BinaryHeap<Scheduled<T>>,
}

impl<T: Timed> TimedQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, item: T) {
        self.queue.push(Scheduled(item));
    }

    pub fn pop_ready(&mut self, now: Instant) -> Box<[T]> {
        let mut ready = Vec::new();

        while self.queue.peek().is_some_and(|timed| timed.0.ready(now)) {
            if let Some(popped) = self.queue.pop() {
                ready.push(popped.0);
            }
        }

        ready.into_boxed_slice()
    }
}
