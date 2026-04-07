//! Contains the Chaff [`Framework`]: an instance of the Chaff library.

use std::time::Instant;

use rand::Rng;

use crate::{
    action::{Action, FrameworkAction, IntegratorAction},
    event::Event,
    machine::{Machine, MachineRuntime},
    queue::{TimedAction, TimedQueue},
    state::TransitionProbs,
};

/// Represents an instance of the Chaff framework.
#[derive(Default, Debug, Clone)]
pub struct Framework<R: Rng> {
    pub(crate) machine: Machine,
    pub(crate) runtime: MachineRuntime,
    rng: R,
}

impl<R: Rng> Framework<R> {
    /// Create a new Chaff instance with the given RNG ([`rand::Rng`]) and [`Machine`].
    pub fn new(machine: Machine, rng: R) -> Self {
        let runtime = MachineRuntime::new(&machine);
        Self {
            machine,
            runtime,
            rng,
        }
    }

    /// Get the [`TransitionProbs`] of the current state as a reference.
    pub fn get_trans_probs(&self) -> Option<&TransitionProbs> {
        // tests do exist for this, but tarpaulin seemingly can't
        // determine whether `.states` or `.trans_probs` are covered
        #[cfg(not(tarpaulin_include))]
        self.machine
            .states
            .get(self.runtime.state)?
            .trans_probs
            .as_ref()
    }

    /// Perform the given [`FrameworkAction`].
    fn perform_action(&mut self, action: FrameworkAction, now: Instant) {
        match action {
            FrameworkAction::Schedule {
                action,
                queue,
                delay,
            } => self.runtime.queues[queue as usize].push(TimedAction {
                execute_at: now + delay,
                action: action.into(),
            }),
            FrameworkAction::CancelQueue(queue) => self.runtime.queues[queue as usize].cancel(),
            FrameworkAction::CancelAll => {
                self.runtime.queues.iter_mut().for_each(TimedQueue::cancel);
            }
        }
    }

    /// Process batch of events and pops scheduled actions off [`MachineRuntime`] queues in that order.
    ///
    /// If a call causes a queue to be popped, a [`Event::QueuePopped`] event will be added to the
    /// [`MachineRuntime`]'s deferred events. The next time [`Framework::process`] is called, these
    /// events will be triggered before the given events.
    ///
    /// Returns only [`IntegratorAction`]s, any [`crate::action::FrameworkAction`]s resulting from
    /// processing events or popping queues will be taken by the [`Framework`] before returning.
    pub fn process(&mut self, events: &[Event], now: Instant) -> Box<[IntegratorAction]> {
        let mut integrator_actions = vec![];
        let mut framework_actions = vec![];
        let mut new_deferred_events = vec![];

        // handle events
        for event in self.runtime.deferred_events.iter().chain(events) {
            if let Some(new_state) = self
                .machine
                .states
                .get(self.runtime.state)
                .and_then(|state| state.trans_probs.as_ref())
                .and_then(|trans_probs| trans_probs.trigger(&mut self.rng, *event))
            {
                self.runtime.state = new_state;

                match self.machine.states[new_state].action {
                    Action::Framework(framework_action) => {
                        framework_actions.push(framework_action);
                    }
                    Action::Integrator(integrator_action) => {
                        integrator_actions.push(integrator_action);
                    }
                }
            }
        }

        // pop queues
        self.runtime
            .pop_queues(now)
            .iter()
            .for_each(|(idx, action)| {
                new_deferred_events.push(Event::QueuePopped(*idx));
                match action {
                    Action::Framework(framework_action) => {
                        framework_actions.push(*framework_action);
                    }
                    Action::Integrator(integrator_action) => {
                        integrator_actions.push(*integrator_action);
                    }
                }
            });

        // set new deferred events
        self.runtime.deferred_events = new_deferred_events;

        // perform framework actions
        for action in framework_actions {
            self.perform_action(action, now);
        }

        integrator_actions.into_boxed_slice()
    }

    /// Get the current state of the frameworks machine.
    pub fn get_state(&self) -> usize {
        self.runtime.state
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::expect_used)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        action::{FrameworkAction, IntegratorAction},
        event::Event,
        machine::Machine,
        state::{State, TransitionProbs},
    };

    #[test]
    fn test_get_trans_probs() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 0.5).try_into().unwrap())]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(
                    Some(trans_probs.clone()),
                    IntegratorAction::SendDecoy.into(),
                ),
                State::new(None, IntegratorAction::SendDecoy.into()),
            ],
            0,
        )
        .unwrap();
        let framework = Framework::new(machine, rand::rng());

        assert_eq!(*framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_and_get_trans_probs() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(
                    Some(trans_probs.clone()),
                    IntegratorAction::SendDecoy.into(),
                ),
                State::new(None, IntegratorAction::SendDecoy.into()),
            ],
            0,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        assert_eq!(*framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);

        framework.process(&[Event::SendNormal], Instant::now());

        assert!(framework.get_trans_probs().is_none());
        assert_eq!(framework.get_state(), 1);
    }

    #[test]
    fn test_trigger_with_0_trans_probs() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 0.0).try_into().unwrap())]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(
                    Some(trans_probs.clone()),
                    IntegratorAction::SendDecoy.into(),
                ),
                State::new(None, FrameworkAction::CancelAll.into()),
            ],
            0,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        assert_eq!(*framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);

        framework.process(&[Event::SendNormal], Instant::now());

        assert_eq!(*framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_get_trans_probs_invalid_state() {
        let machine = Machine::new(
            vec![State::new(None, IntegratorAction::SendDecoy.into())],
            0,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        // force bad behaviour
        framework.runtime.state = 999;

        assert!(framework.get_trans_probs().is_none());
    }

    #[test]
    fn test_get_trans_probs_state_with_no_trans_probs() {
        let machine = Machine::new(
            vec![State::new(None, IntegratorAction::SendDecoy.into())],
            0,
        )
        .unwrap();

        let framework = Framework::new(machine, rand::rng());

        assert!(framework.get_trans_probs().is_none());
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_events_no_matching_event() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(None, IntegratorAction::SendDecoy.into()),
            ],
            0,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let actions = framework.process(&[Event::ReceiveNormal], Instant::now());

        assert!(actions.is_empty());
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_perform_action_cancel_all() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(None, FrameworkAction::CancelAll.into()),
            ],
            2,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let far_future = Instant::now() + std::time::Duration::from_secs(9999);
        for queue in &mut framework.runtime.queues {
            queue.push(TimedAction {
                execute_at: far_future,
                action: IntegratorAction::SendDecoy.into(),
            });
        }

        framework.process(&[Event::SendNormal], Instant::now());

        for queue in &framework.runtime.queues {
            assert!(queue.is_empty());
        }
    }

    #[test]
    fn test_perform_action_cancel_queue() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(None, FrameworkAction::CancelQueue(0).into()),
            ],
            2,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let far_future = Instant::now() + std::time::Duration::from_secs(999);
        for queue in &mut framework.runtime.queues {
            queue.push(TimedAction {
                execute_at: far_future,
                action: IntegratorAction::SendDecoy.into(),
            });
        }

        framework.process(&[Event::SendNormal], Instant::now());

        assert!(
            framework.runtime.queues[0].is_empty(),
            "queue 0 should have been cancelled"
        );

        assert!(
            !framework.runtime.queues[1].is_empty(),
            "other queues should not have been affected"
        );
    }

    #[test]
    fn test_perform_action_schedule() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::ReleaseBlock.into()),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 0,
                        delay: Duration::from_secs(999),
                    }
                    .into(),
                ),
            ],
            1,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let actions = framework.process(&[Event::SendNormal], Instant::now());

        assert!(actions.is_empty());

        let queued = framework.runtime.queues[0]
            .queue
            .pop()
            .expect("expected something to be queued.");

        assert_eq!(queued.0.action, IntegratorAction::SendDecoy.into());
    }

    #[test]
    fn test_perform_action_schedule_far_doesnt_pop() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(
                    None,
                    FrameworkAction::Schedule {
                        action: IntegratorAction::SendDecoy,
                        queue: 0,
                        delay: Duration::from_secs(999),
                    }
                    .into(),
                ),
            ],
            1,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let actions = framework.process(&[Event::SendNormal], Instant::now());

        assert!(
            !actions.contains(&IntegratorAction::SendDecoy),
            "far future action should not fire yet"
        );
        assert!(
            !framework.runtime.queues[0].is_empty(),
            "action should still be sitting in the queue"
        );
    }

    #[test]
    fn test_perform_action_cancel_all_via_queue() {
        let machine = Machine::new(
            vec![State::new(None, IntegratorAction::SendDecoy.into())],
            3,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::CancelAll.into(),
        });

        let far_future = now + Duration::from_secs(999);
        for queue in &mut framework.runtime.queues[1..] {
            queue.push(TimedAction {
                execute_at: far_future,
                action: IntegratorAction::SendDecoy.into(),
            });
        }

        framework.process(&[], now);

        for queue in &framework.runtime.queues {
            assert!(queue.is_empty(), "queues should have been cancelled");
        }
    }

    #[test]
    fn test_perform_action_cancel_queue_via_queue() {
        let machine = Machine::new(
            vec![State::new(None, IntegratorAction::SendDecoy.into())],
            2,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::CancelQueue(0).into(),
        });

        let far_future = now + Duration::from_secs(9999);
        framework.runtime.queues[1].push(TimedAction {
            execute_at: far_future,
            action: IntegratorAction::SendDecoy.into(),
        });

        framework.process(&[], now);

        assert!(
            framework.runtime.queues[0].is_empty(),
            "queue 0 should have been cancelled"
        );

        assert!(
            !framework.runtime.queues[1].is_empty(),
            "other queues should not have been affected"
        );
    }

    #[test]
    fn test_perform_action_schedule_via_queue() {
        let machine = Machine::new(
            vec![State::new(None, IntegratorAction::SendDecoy.into())],
            2,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();

        framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::Schedule {
                action: IntegratorAction::SendDecoy,
                queue: 1,
                delay: Duration::ZERO,
            }
            .into(),
        });

        // queues should get popped, schedule action should be processed as it
        // was already on the queue at this instant
        let actions = framework.process(&[], now);

        assert!(actions.is_empty());

        // now we can process again (with the same now) which should pop the
        // scheduled action
        let actions = framework.process(&[], now);

        assert!(!actions.is_empty());
        assert_eq!(actions[0], IntegratorAction::SendDecoy);
    }

    #[test]
    fn test_deferred_queue_popped_event_ordering() {
        let trans_probs =
            TransitionProbs::new([(Event::QueuePopped(0), (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy.into()),
                State::new(None, IntegratorAction::ReleaseBlock.into()),
            ],
            1,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        let actions = framework.process(&[], now);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], IntegratorAction::SendDecoy);
        assert_eq!(
            framework.get_state(),
            0,
            "state should not change in the same tick as the pop"
        );
        assert_eq!(
            framework.runtime.deferred_events.len(),
            1,
            "queue pop event should be deferred"
        );

        let actions = framework.process(&[], now);

        assert_eq!(
            framework.get_state(),
            1,
            "state should have transitioned in the second call"
        );

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], IntegratorAction::ReleaseBlock);
        assert!(framework.runtime.deferred_events.is_empty());
    }

    #[test]
    fn test_deferred_events_happen_before_new_events() {
        let trans_0_to_1 =
            TransitionProbs::new([(Event::QueuePopped(0), (1, 1.0).try_into().unwrap())]).unwrap();
        let trans_1_to_2 =
            TransitionProbs::new([(Event::SendNormal, (2, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_0_to_1), IntegratorAction::SendDecoy.into()),
                State::new(Some(trans_1_to_2), IntegratorAction::SendDecoy.into()),
                State::new(None, IntegratorAction::ReleaseBlock.into()),
            ],
            1,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        framework.process(&[], now);
        framework.process(&[Event::SendNormal], now);

        // can only reach state 2 via state 1. can only reach state 1 for the Event::SendNormal
        // trigger to transition to state 2 if deferred event processed first.
        assert_eq!(
            framework.get_state(),
            2,
            "should have processed deferred event then new event to reach state 2"
        );
    }
}
