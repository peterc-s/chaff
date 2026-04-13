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
                action: int_action,
                queue,
                delay,
            } => {
                if !self.runtime.queues[queue as usize].push(TimedAction {
                    execute_at: now + delay.sample_dyn(&mut self.rng),
                    action: int_action.into(),
                }) {
                    self.runtime.deferred_events.push(Event::QueueFull(queue));
                }
            }
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
    ///
    /// Entering states with `0` budget will immediately cause a deferred [`Event::StateBudgetExhausted`]
    /// event to be emitted by the framework.
    pub fn process(&mut self, events: &[Event], now: Instant) -> Box<[IntegratorAction]> {
        let mut integrator_actions = vec![];
        let mut framework_actions = vec![];
        let mut new_deferred_events = vec![];

        // in a separate closure as the budget handling should be consistent between handling events
        // and popping queues.

        // TODO: this is a bit unclear and also it isn't clear how it will affect integrators.
        // should probably document it and think a bit more about the interaction.
        let handle_integrator_action =
            |action: &IntegratorAction,
             current_budget: &mut Option<usize>,
             new_deferred_events: &mut Vec<Event>,
             integrator_actions: &mut Vec<IntegratorAction>| {
                if matches!(action, IntegratorAction::SendDecoy)
                    && let Some(budget) = current_budget
                {
                    if *budget == 0 {
                        return;
                    }
                    *budget = budget.saturating_sub(1);
                    if *budget == 0 {
                        new_deferred_events.push(Event::StateBudgetExhausted);
                    }
                }
                integrator_actions.push(action.clone());
            };

        // handle events
        for event in self.runtime.deferred_events.iter().chain(events) {
            if let Some(new_state) = self
                .machine
                .states
                .get(self.runtime.state)
                .and_then(|state| state.trans_probs.as_ref())
                .and_then(|trans_probs| trans_probs.trigger(&mut self.rng, *event))
            {
                let is_new_state = self.runtime.state != new_state;
                self.runtime.state = new_state;

                if is_new_state {
                    self.runtime.current_budget = self.machine.states[new_state].decoy_budget;
                    if self.runtime.current_budget == Some(0) {
                        new_deferred_events.push(Event::StateBudgetExhausted);
                    }
                }

                match &self.machine.states[new_state].action {
                    Action::Framework(framework_action) => {
                        framework_actions.push(framework_action.clone());
                    }
                    Action::Integrator(integrator_action) => {
                        handle_integrator_action(
                            integrator_action,
                            &mut self.runtime.current_budget,
                            &mut new_deferred_events,
                            &mut integrator_actions,
                        );
                    }
                }
            }
        }

        // pop queues
        let queue_popped_binding = self.runtime.pop_queues(now);
        queue_popped_binding.iter().for_each(|(idx, action)| {
            new_deferred_events.push(Event::QueuePopped(*idx));
            match action {
                Action::Framework(framework_action) => {
                    framework_actions.push(framework_action.clone());
                }
                Action::Integrator(integrator_action) => {
                    handle_integrator_action(
                        integrator_action,
                        &mut self.runtime.current_budget,
                        &mut new_deferred_events,
                        &mut integrator_actions,
                    );
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
        machine,
        machine::Machine,
        state::{State, TransitionProbs},
    };

    #[test]
    fn test_get_trans_probs() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, [(1, 0.5).try_into().unwrap()])]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs.clone()), IntegratorAction::SendDecoy, None),
                State::new(None, IntegratorAction::SendDecoy, None),
            ],
            [],
        )
        .unwrap();
        let framework = Framework::new(machine, rand::rng());

        assert_eq!(*framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_and_get_trans_probs() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs.clone()), IntegratorAction::SendDecoy, None),
                State::new(None, IntegratorAction::SendDecoy, None),
            ],
            [],
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
            TransitionProbs::new([(Event::SendNormal, [(1, 0.0).try_into().unwrap()])]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs.clone()), IntegratorAction::SendDecoy, None),
                State::new(None, FrameworkAction::CancelAll, None),
            ],
            [],
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
            vec![State::new(None, IntegratorAction::SendDecoy, None)],
            [],
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
            vec![State::new(None, IntegratorAction::SendDecoy, None)],
            [],
        )
        .unwrap();

        let framework = Framework::new(machine, rand::rng());

        assert!(framework.get_trans_probs().is_none());
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_events_no_matching_event() {
        let trans_probs =
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy, None),
                State::new(None, IntegratorAction::SendDecoy, None),
            ],
            [],
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
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy, None),
                State::new(None, FrameworkAction::CancelAll, None),
            ],
            [None, None],
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let far_future = Instant::now() + std::time::Duration::from_secs(9999);
        for queue in &mut framework.runtime.queues {
            let _ = queue.push(TimedAction {
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
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();
        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy, None),
                State::new(None, FrameworkAction::CancelQueue(0), None),
            ],
            [None, None],
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let far_future = Instant::now() + std::time::Duration::from_secs(999);
        for queue in &mut framework.runtime.queues {
            let _ = queue.push(TimedAction {
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
        let machine = machine! {
            queues: [None],
            state init {
                action: IntegratorAction::ReleaseBlock,
                transitions: [Event::SendNormal => schedule_decoy],
            },
            state schedule_decoy {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    Duration::from_secs(999)
                ),
            }
        }
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
            TransitionProbs::new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_probs), IntegratorAction::SendDecoy, None),
                State::new(
                    None,
                    FrameworkAction::schedule(
                        IntegratorAction::SendDecoy,
                        0,
                        Duration::from_secs(999),
                    ),
                    None,
                ),
            ],
            [None],
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
            vec![State::new(None, IntegratorAction::SendDecoy, None)],
            [None; 3],
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::CancelAll.into(),
        });

        let far_future = now + Duration::from_secs(999);
        for queue in &mut framework.runtime.queues[1..] {
            let _ = queue.push(TimedAction {
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
            vec![State::new(None, IntegratorAction::SendDecoy, None)],
            [None, None],
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::CancelQueue(0).into(),
        });

        let far_future = now + Duration::from_secs(9999);
        let _ = framework.runtime.queues[1].push(TimedAction {
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
            vec![State::new(None, IntegratorAction::SendDecoy, None)],
            [None, None],
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();

        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::schedule(IntegratorAction::SendDecoy, 1, Duration::ZERO)
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
        let machine = machine! {
            queues: [None],
            state wait_pop {
                action: IntegratorAction::SendDecoy,
                transitions: [Event::QueuePopped(0) => release],
            },
            state release {
                action: IntegratorAction::ReleaseBlock
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let _ = framework.runtime.queues[0].push(TimedAction {
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
        let machine = machine! {
            queues: [None],
            state init {
                action: IntegratorAction::SendDecoy,
                transitions: [Event::QueuePopped(0) => jump],
            },
            state jump {
                action: IntegratorAction::SendDecoy,
                transitions: [Event::SendNormal => end],
            },
            state end {
                action: IntegratorAction::ReleaseBlock,
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        framework.process(&[], now);
        framework.process(&[Event::SendNormal], now);

        // can only reach state end via state jump. can only reach state jump for the Event::SendNormal
        // trigger to transition to state end if deferred event processed first.
        assert_eq!(
            framework.get_state(),
            2,
            "should have processed deferred event then new event to reach state 2"
        );
    }

    #[test]
    fn test_framework_action_schedule_samples_delay() {
        let machine = Machine::new(
            vec![State::new(None, IntegratorAction::ReleaseBlock, None)],
            [None],
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let action =
            FrameworkAction::schedule(IntegratorAction::SendDecoy, 0, Duration::from_secs(5));

        framework.perform_action(action, Instant::now());
        assert_eq!(framework.runtime.queues[0].queue.len(), 1);
    }

    #[test]
    fn test_queue_capacity_limit() {
        let machine = machine! {
            queues: [Some(1)],
            state init {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    Duration::from_secs(999)
                ),
                transitions: [
                    Event::ReceiveNormal => init,
                    Event::SendNormal => overflow,
                ],
            },
            state overflow {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    Duration::from_secs(999)
                ),
                transitions: [Event::QueueFull(0) => end],
            },
            state end {
                action: IntegratorAction::ReleaseBlock,
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        // trigger `init`'s action
        framework.process(&[Event::ReceiveNormal], now);
        assert_eq!(framework.runtime.queues[0].queue.len(), 1);

        // transition to `overflow`
        framework.process(&[Event::SendNormal], now);
        assert_eq!(framework.runtime.queues[0].queue.len(), 1);

        // queuefull should trigger and we should be in `end`
        framework.process(&[], now);
        assert_eq!(framework.get_state(), 2);
    }

    #[test]
    fn test_state_budget_exhausted() {
        let machine = machine! {
            queues: [],
            state init {
                action: IntegratorAction::ReleaseBlock,
                transitions: [Event::SendNormal => decoy_burst],
            },
            state decoy_burst {
                action: IntegratorAction::SendDecoy,
                transitions: [Event::StateBudgetExhausted => end],
                budget: 1,
            },
            state end {
                action: IntegratorAction::ReleaseBlock
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        // transition to `decoy_burst`
        let actions = framework.process(&[Event::SendNormal], now);
        assert!(actions.contains(&IntegratorAction::SendDecoy));
        assert_eq!(framework.get_state(), 1);

        // deferred state budget exhausted event should cause transition to `end`
        framework.process(&[], now);
        assert_eq!(framework.get_state(), 2);
    }

    #[test]
    fn test_state_budget_exhausted_via_queue() {
        let machine = machine! {
            queues: [None],
            state init {
                action: IntegratorAction::ReleaseBlock,
                transitions: [Event::StateBudgetExhausted => end],
                budget: 1,
            },
            state end {
                action: IntegratorAction::ReleaseBlock
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        let actions = framework.process(&[], now);
        assert!(actions.contains(&IntegratorAction::SendDecoy));
        assert_eq!(framework.get_state(), 0);

        framework.process(&[], now);
        assert_eq!(framework.get_state(), 1);
    }

    #[test]
    fn test_state_budget_zero_and_self_transitions() {
        let machine = machine! {
            queues: [],
            state init {
                action: IntegratorAction::ReleaseBlock,
                transitions: [Event::ReceiveNormal => decoy_burst],
            },
            state decoy_burst {
                action: IntegratorAction::SendDecoy,
                transitions: [
                    Event::SendNormal => decoy_burst,
                    Event::StateBudgetExhausted => end
                ],
                budget: 2,
            },
            state end {
                action: IntegratorAction::SendDecoy,
                budget: 0,
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        // transition to `decoy_burst`
        let actions = framework.process(&[Event::ReceiveNormal], now);
        assert!(actions.contains(&IntegratorAction::SendDecoy));
        assert_eq!(framework.get_state(), 1);

        // self-transition, should exhaust budget
        let actions = framework.process(&[Event::SendNormal], now);
        assert!(actions.contains(&IntegratorAction::SendDecoy));
        assert_eq!(framework.get_state(), 1);

        // deferred exhaustion should transition to end, the send decoy should
        // be blocked because `end` has a budget of 0.
        let actions = framework.process(&[], now);
        assert_eq!(framework.get_state(), 2);
        assert!(actions.is_empty());
    }
}
