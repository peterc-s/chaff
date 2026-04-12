//! Contains the chaff [`TimedQueue`] which is forms part of a [`crate::machine::MachineRuntime`].

use std::{collections::BinaryHeap, time::Instant};

use crate::action::Action;

/// Represents that something is "timed": it can be "ready" and will "execute" at some [`Instant`].
pub trait Timed {
    /// Returns whether it is ready.
    fn ready(&self, now: Instant) -> bool;

    /// Returns the instant at which it should be ready.
    fn execute_at(&self) -> Instant;
}

/// A wrapper around implementors of [`Timed`], representing that something can be scheduled.
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

/// An [`Action`] that implements [`Timed`] for use with the [`TimedQueue`].
#[derive(Debug, Clone)]
pub struct TimedAction {
    /// When to execute the action (see [`Timed::execute_at`]).
    pub execute_at: Instant,

    /// The [`Action`] to execute.
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

/// A priority queue of [`Scheduled`] objects, ordered by their [`Timed::execute_at`]. Under the hood
/// this is just a [`BinaryHeap<Scheduled<T>>`].
#[derive(Debug, Clone, Default)]
pub struct TimedQueue<T: Timed> {
    pub(crate) queue: BinaryHeap<Scheduled<T>>,
}

impl<T: Timed> TimedQueue<T> {
    /// Create a new, empty [`TimedQueue`].
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }

    /// Push an item onto the [`TimedQueue`].
    pub fn push(&mut self, item: T) {
        self.queue.push(Scheduled(item));
    }

    /// Pop all items that are ready (i.e. [`Timed::ready`] is `true`)
    pub fn pop_ready(&mut self, now: Instant) -> Box<[T]> {
        let mut ready = Vec::new();

        while self.queue.peek().is_some_and(|timed| timed.0.ready(now)) {
            if let Some(popped) = self.queue.pop() {
                ready.push(popped.0);
            }
        }

        ready.into_boxed_slice()
    }

    /// Cancel all scheduled events on the queue.
    pub fn cancel(&mut self) {
        self.queue.clear();
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::time::Duration;

    use crate::action::IntegratorAction;

    use super::*;

    #[test]
    fn test_scheduled_eq() {
        let now = Instant::now();
        let timed_action = TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: now,
        };

        let scheduled_0 = Scheduled(timed_action.clone());
        let scheduled_1 = Scheduled(timed_action);

        assert_eq!(scheduled_0, scheduled_1);

        let timed_action = TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: now + Duration::from_secs(1),
        };

        let scheduled_1 = Scheduled(timed_action);

        assert_ne!(scheduled_0, scheduled_1);
    }

    #[test]
    fn test_scheduled_ord_and_partial_ord() {
        let now = Instant::now();
        let soon = now + Duration::from_secs(1);
        let later = now + Duration::from_secs(10);

        let scheduled_soon = Scheduled(TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: soon,
        });
        let scheduled_later = Scheduled(TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: later,
        });

        assert_eq!(
            scheduled_soon.cmp(&scheduled_later),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            scheduled_later.cmp(&scheduled_soon),
            std::cmp::Ordering::Less
        );

        assert!(scheduled_soon > scheduled_later);
        assert!(scheduled_later < scheduled_soon);
    }

    #[test]
    fn test_min_heap_behavior() {
        let now = Instant::now();
        let earliest = now + Duration::from_secs(1);
        let latest = now + Duration::from_secs(10);
        let mut heap = BinaryHeap::new();

        heap.push(Scheduled(TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: latest,
        }));
        heap.push(Scheduled(TimedAction {
            action: IntegratorAction::SendDecoy.into(),
            execute_at: earliest,
        }));

        let popped = heap.pop().unwrap();
        assert_eq!(popped.0.execute_at, earliest);
    }
}
