//! Contains the Chaff [`Framework`]: an instance of the Chaff library.

use std::time::Instant;

use rand::{CryptoRng, Rng};
use rand_distr::Distribution as _;

use crate::{
    action::{Action, FrameworkAction, IntegratorAction},
    event::Event,
    machine::{Machine, MachineDecoyBudget, MachineRuntime},
    queue::{QueuePushStatus, TimedAction, TimedQueue},
    state::TransitionProbs,
};

/// Represents an instance of the Chaff framework.
#[derive(Debug, Clone)]
pub struct Framework<R: Rng + CryptoRng> {
    pub(crate) machine: Machine,
    pub(crate) runtime: MachineRuntime,
    rng: R,
}

impl<R: Rng + CryptoRng> Framework<R> {
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
        self.machine.states[self.runtime.state].trans_probs.as_ref()
    }

    /// Peeks the [`Instant`] of the soonest scheduled [`TimedAction`] in the [`MachineRuntime`]s
    /// [`TimedQueue`]s.
    pub fn peek_soonest_scheduled_instant(&self) -> Option<Instant> {
        self.runtime.peek_soonest_scheduled_instant()
    }

    /// Checks if the [`MachineRuntime`] has `initialised`.
    pub fn is_initialised(&self) -> bool {
        self.runtime.initialised
    }

    /// Perform the given [`FrameworkAction`].
    fn perform_action(&mut self, action: FrameworkAction, now: Instant) {
        match action {
            FrameworkAction::Schedule {
                action: int_action,
                queue,
                delay,
            } => {
                match self.runtime.queues[queue as usize].push(TimedAction {
                    execute_at: now + delay.sample(&mut self.rng),
                    action: int_action.into(),
                }) {
                    QueuePushStatus::Pushed => {
                        self.runtime.deferred_events.push(Event::QueuePushed(queue));
                    }
                    QueuePushStatus::PushedButFull => {
                        self.runtime.deferred_events.push(Event::QueueFilled(queue));
                    }
                    QueuePushStatus::Full => {
                        self.runtime.deferred_events.push(Event::QueueFull(queue));
                    }
                }
            }
            FrameworkAction::CancelQueue(queue) => self.runtime.queues[queue as usize].cancel(),
            FrameworkAction::CancelAll => {
                self.runtime.queues.iter_mut().for_each(TimedQueue::cancel);
            }
        }
    }

    /// Apply the current state's decoy budget to a [`IntegratorAction`], returning `false` if it
    /// should be suppressed. Machine budget takes precedence over state budget.
    fn apply_budget(&mut self, action: &IntegratorAction) -> bool {
        if matches!(action, IntegratorAction::SendDecoy) {
            // machine
            if let Some(budget) = &self.machine.budget {
                match budget {
                    MachineDecoyBudget::Absolute(max) => {
                        if self.runtime.decoys_sent >= *max {
                            return false;
                        }

                        if self.runtime.decoys_sent + 1 == *max {
                            self.runtime
                                .deferred_events
                                .push(Event::MachineBudgetExhausted);
                        }
                    }
                    MachineDecoyBudget::Proportion(proportion) => {
                        // block if no real packets seen yet
                        if self.runtime.real_sent == 0 {
                            if !self.runtime.proportion_blocked {
                                self.runtime
                                    .deferred_events
                                    .push(Event::MachineBudgetReached);
                                self.runtime.proportion_blocked = true;
                            }
                            return false;
                        }

                        #[expect(clippy::cast_precision_loss)]
                        #[expect(clippy::cast_possible_truncation)]
                        #[expect(clippy::cast_sign_loss)]
                        let allowed = (proportion * self.runtime.real_sent as f64).ceil() as usize;

                        // if we're already over the budget then suppress
                        if self.runtime.decoys_sent >= allowed {
                            if !self.runtime.proportion_blocked {
                                self.runtime
                                    .deferred_events
                                    .push(Event::MachineBudgetReached);
                                self.runtime.proportion_blocked = true;
                            }
                            return false;
                        }

                        // if we're just about to hit the budget, then emit an event
                        if self.runtime.decoys_sent + 1 == allowed {
                            self.runtime
                                .deferred_events
                                .push(Event::MachineBudgetReached);
                            self.runtime.proportion_blocked = true;
                        }
                    }
                }
            }

            // state
            match &mut self.runtime.current_state_budget {
                Some(budget) if *budget == 0 => return false,
                Some(budget) => {
                    *budget -= 1;
                    if *budget == 0 {
                        self.runtime
                            .deferred_events
                            .push(Event::StateBudgetExhausted);
                    }
                }
                None => {}
            }

            self.runtime.decoys_sent += 1;
        }
        true
    }

    /// Collect all [`Action`]s and deferred [`Event`]s. Covers initialisation, triggered events, and queue popping.
    fn collect_actions(&mut self, events: &[Event], now: Instant) -> (Vec<Action>, Vec<Event>) {
        let mut actions = vec![];
        let mut deferred = vec![];

        // initialisation
        if !self.runtime.initialised {
            if let Some(action) = &self.machine.states[self.runtime.state].action {
                actions.push(action.clone());
            }
            self.runtime.initialised = true;
        }

        // handle deferred + triggered events
        let prior_deferred = std::mem::take(&mut self.runtime.deferred_events);
        for event in prior_deferred.iter().chain(events) {
            // handle machine budgeting
            if *event == Event::SendNormal {
                self.runtime.real_sent += 1;
                if let Some(MachineDecoyBudget::Proportion(proportion)) = &self.machine.budget {
                    #[expect(clippy::cast_precision_loss)]
                    #[expect(clippy::cast_possible_truncation)]
                    #[expect(clippy::cast_sign_loss)]
                    let allowed = (proportion * self.runtime.real_sent as f64).ceil() as usize;
                    if self.runtime.proportion_blocked && allowed > self.runtime.decoys_sent {
                        deferred.push(Event::MachineBudgetRecovered);
                        self.runtime.proportion_blocked = false;
                    }
                }
            }

            // handle transition
            if let Some(new_state) = self.machine.states[self.runtime.state]
                .trans_probs
                .as_ref()
                .and_then(|trans_probs| trans_probs.trigger(&mut self.rng, *event))
            {
                if self.runtime.state != new_state {
                    self.runtime.state = new_state;
                    self.runtime.current_state_budget = self.machine.states[new_state].decoy_budget;
                    if self.runtime.current_state_budget == Some(0) {
                        deferred.push(Event::StateBudgetExhausted);
                    }
                }

                if let Some(action) = &self.machine.states[new_state].action {
                    actions.push(action.clone());
                }
            }
        }

        // pop queues
        for (idx, action) in self.runtime.pop_queues(now) {
            deferred.push(Event::QueuePopped(idx));
            if self.runtime.queues[idx as usize].is_empty() {
                deferred.push(Event::QueueEmpty(idx));
            }
            actions.push(action);
        }

        (actions, deferred)
    }

    /// Process batch of events and pops scheduled actions off [`MachineRuntime`] queues in that order.
    ///
    /// If a call causes a queue to be popped, a [`Event::QueuePopped`] event will be added to the
    /// [`MachineRuntime`]'s deferred events. The next time [`Framework::process`] is called, these
    /// events will be triggered before the given events.
    ///
    /// Returns only [`IntegratorAction`]s, any [`FrameworkAction`]s resulting from
    /// processing events or popping queues will be taken by the [`Framework`] before returning.
    ///
    /// Entering states with `0` budget will immediately cause a deferred [`Event::StateBudgetExhausted`]
    /// event to be emitted by the framework.
    pub fn process(&mut self, events: &[Event], now: Instant) -> Box<[IntegratorAction]> {
        let (actions, deferred) = self.collect_actions(events, now);
        self.runtime.deferred_events = deferred;

        actions
            .into_iter()
            .filter_map(|action| match action {
                Action::Framework(a) => {
                    self.perform_action(a, now);
                    None
                }
                Action::Integrator(a) => {
                    if self.apply_budget(&a) {
                        Some(a)
                    } else {
                        None
                    }
                }
            })
            .collect()
    }

    /// Get the current state of the frameworks machine.
    pub fn get_state(&self) -> usize {
        self.runtime.state
    }

    /// Clones the current [`MachineRuntime`].
    pub fn peek_runtime(&self) -> &MachineRuntime {
        &self.runtime
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
        distr::DistrKind,
        event::Event,
        machine,
        machine::Machine,
        state::{State, TransitionProbs},
    };

    #[test]
    fn test_get_trans_probs() {
        let trans_probs =
            TransitionProbs::try_new([(Event::SendNormal, [(1, 0.5).try_into().unwrap()])])
                .unwrap();
        let machine = Machine::try_new(
            vec![
                State::new(
                    Some(trans_probs.clone()),
                    Some(IntegratorAction::SendDecoy),
                    None,
                ),
                State::new(None, Some(IntegratorAction::SendDecoy), None),
            ],
            [],
            None,
        )
        .unwrap();
        let framework = Framework::new(machine, rand::rng());

        assert_eq!(*framework.get_trans_probs().unwrap(), trans_probs);
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_and_get_trans_probs() {
        let trans_probs =
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();
        let machine = Machine::try_new(
            vec![
                State::new(
                    Some(trans_probs.clone()),
                    Some(IntegratorAction::SendDecoy),
                    None,
                ),
                State::new(None, Some(IntegratorAction::SendDecoy), None),
            ],
            [],
            None,
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
            TransitionProbs::try_new([(Event::SendNormal, [(1, 0.0).try_into().unwrap()])])
                .unwrap();
        let machine = Machine::try_new(
            vec![
                State::new(
                    Some(trans_probs.clone()),
                    Some(IntegratorAction::SendDecoy),
                    None,
                ),
                State::new(None, Some(FrameworkAction::CancelAll), None),
            ],
            [],
            None,
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
    fn test_get_trans_probs_state_with_no_trans_probs() {
        let machine = Machine::try_new(
            vec![State::new(None, Some(IntegratorAction::SendDecoy), None)],
            [],
            None,
        )
        .unwrap();

        let framework = Framework::new(machine, rand::rng());

        assert!(framework.get_trans_probs().is_none());
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_trigger_events_no_matching_event() {
        let trans_probs =
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), None::<Action>, None),
                State::new(None, Some(IntegratorAction::SendDecoy), None),
            ],
            [],
            None,
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
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), Some(IntegratorAction::SendDecoy), None),
                State::new(None, Some(FrameworkAction::CancelAll), None),
            ],
            [None, None],
            None,
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
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();
        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), Some(IntegratorAction::SendDecoy), None),
                State::new(None, Some(FrameworkAction::CancelQueue(0)), None),
            ],
            [None, None],
            None,
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
                transitions: [Event::SendNormal => schedule_decoy],
            },
            state schedule_decoy {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    DistrKind::Constant(999.0).try_into().unwrap()
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
            TransitionProbs::try_new([(Event::SendNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans_probs), None::<Action>, None),
                State::new(
                    None,
                    Some(FrameworkAction::schedule(
                        IntegratorAction::SendDecoy,
                        0,
                        DistrKind::Constant(999.0).try_into().unwrap(),
                    )),
                    None,
                ),
            ],
            [None],
            None,
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
        let machine = Machine::try_new(
            vec![State::new(None, Some(IntegratorAction::SendDecoy), None)],
            [None; 3],
            None,
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
        let machine = Machine::try_new(
            vec![State::new(None, Some(IntegratorAction::SendDecoy), None)],
            [None, None],
            None,
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
        let machine = Machine::try_new(
            vec![State::new(None, None::<Action>, None)],
            [None, None],
            None,
        )
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();

        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: FrameworkAction::schedule(
                IntegratorAction::SendDecoy,
                1,
                DistrKind::Constant(0.0).try_into().unwrap(),
            )
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
            framework.runtime.deferred_events,
            [Event::QueuePopped(0), Event::QueueEmpty(0)],
            "queue pop and queue empty event should be deferred"
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
        let machine = Machine::try_new(
            vec![State::new(None, Some(IntegratorAction::ReleaseBlock), None)],
            [None],
            None,
        )
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let action = FrameworkAction::schedule(
            IntegratorAction::SendDecoy,
            0,
            DistrKind::Constant(5.0).try_into().unwrap(),
        );

        framework.perform_action(action, Instant::now());
        assert_eq!(framework.runtime.queues[0].queue.len(), 1);
    }

    #[test]
    fn test_queue_capacity_limit() {
        let long_delay = DistrKind::Constant(999.0).try_into().unwrap();
        let machine = machine! {
            queues: [Some(1)],
            state init {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    long_delay,
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
                    long_delay,
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

    #[test]
    fn test_queue_empty() {
        let machine = machine! {
            queues: [Some(2)],
            state init {
                action: IntegratorAction::ReleaseBlock,
                transitions: [Event::ReceiveNormal => fill_queue],
            },
            state fill_queue {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    DistrKind::Constant(Duration::from_millis(15).as_secs_f64()).try_into().unwrap(),
                ),
                transitions: [
                    Event::SendNormal => fill_queue,
                    Event::ReceiveNormal => wait_for_queue_drain
                ],
            },
            state wait_for_queue_drain {
                action: IntegratorAction::ReleaseBlock,
                transitions: [Event::QueueEmpty(0) => end],
            },
            state end {
                action: IntegratorAction::ReleaseBlock,
            }
        }
        .unwrap();

        let mut framework = Framework::new(machine, rand::rng());
        let now = Instant::now();

        framework.process(&[Event::ReceiveNormal], now);
        assert_eq!(framework.get_state(), 1);

        framework.process(&[Event::SendNormal, Event::SendNormal], now);
        assert_eq!(framework.get_state(), 1);

        framework.process(&[Event::ReceiveNormal], now);
        assert_eq!(framework.get_state(), 2);

        // drain queue
        let now = now + Duration::from_millis(16);
        let actions = framework.process(&[], now);
        assert_eq!(
            *actions,
            [IntegratorAction::SendDecoy, IntegratorAction::SendDecoy]
        );
        assert_eq!(framework.get_state(), 2);

        // deferred event should cause transition
        framework.process(&[], now);
        assert_eq!(framework.get_state(), 3);
    }

    #[test]
    fn test_none_action() {
        let machine = machine! {
            queues: [None],
            state init {
                transitions: [Event::SendNormal => end],
            },
            state end {},
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let actions = framework.process(&[Event::SendNormal], Instant::now());

        assert!(actions.is_empty());
        assert!(framework.runtime.queues[0].queue.is_empty());
    }

    #[test]
    fn test_no_transition() {
        let machine = machine! {
            queues: [None],
            state init {
                transitions: [
                    Event::ReceiveNormal => [(end, 0.0)],
                    Event::SendNormal => end,
                ],
            },
            state end {},
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let actions = framework.process(&[Event::ReceiveNormal], Instant::now());

        assert!(actions.is_empty());
        assert!(framework.runtime.queues[0].queue.is_empty());
        assert_eq!(framework.get_state(), 0);

        let actions = framework.process(&[Event::SendDecoy], Instant::now());

        assert!(actions.is_empty());
        assert!(framework.runtime.queues[0].queue.is_empty());
        assert_eq!(framework.get_state(), 0);
    }

    #[test]
    fn test_absolute_machine_budget_exhaustion_and_suppression() {
        // attempt to send immediately
        let machine = machine! {
            queues: [None],
            budget: Absolute(1),
            state init {
                action: IntegratorAction::SendDecoy,
            },
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let actions = framework.process(&[], now);

        // initialisation will attempt a send, machine budget exhaustion should be deferred
        assert!(actions.contains(&IntegratorAction::SendDecoy));
        assert_eq!(framework.runtime.decoys_sent, 1);
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetExhausted)
        );

        // attempt a second send
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        // queued decoy should be popped and suppressed by absolute budget
        let actions = framework.process(&[], now);
        assert!(actions.is_empty());
        assert_eq!(framework.runtime.decoys_sent, 1);

        // ensure MachineBudgetExhausted is not repeatedly sent
        assert!(
            !framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetExhausted)
        );
    }

    #[test]
    fn test_proportion_block_when_no_real_and_recovery() {
        let machine = machine! {
            queues: [None],
            budget: Proportion(0.5),
            state init {
                action: IntegratorAction::SendDecoy,
            },
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        // for proportional limits we block and suppress if no real packets sent first
        let now = Instant::now();
        let actions = framework.process(&[], now);
        assert!(actions.is_empty(), "decoy should be blocked");
        assert!(
            framework.runtime.proportion_blocked,
            "proportion_blocked should be set"
        );
        assert_eq!(framework.runtime.decoys_sent, 0);
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached)
        );
        assert!(
            !framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetRecovered),
        );

        // simulate a real send for recovery
        let _ = framework.process(&[Event::SendNormal], now);
        assert!(framework.runtime.real_sent == 1);
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetRecovered),
        );

        // send a decoy
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });
        let actions = framework.process(&[], now);
        assert_eq!(framework.runtime.decoys_sent, 1);
        assert!(actions.contains(&IntegratorAction::SendDecoy));
    }

    #[test]
    fn test_state_budget_not_consumed_when_machine_blocks() {
        let machine = machine! {
            queues: [],
            budget: Absolute(0),
            state init {
                action: IntegratorAction::SendDecoy,
                budget: 1
            },
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        // decoy should be blocked by machine before state budget is consumed
        let now = Instant::now();
        let actions = framework.process(&[], now);
        assert!(actions.is_empty());
        assert_eq!(framework.runtime.current_state_budget, Some(1));
        assert!(
            !framework
                .runtime
                .deferred_events
                .contains(&Event::StateBudgetExhausted)
        );

        // TODO: should this happen?
        // assert!(
        //     framework
        //         .runtime
        //         .deferred_events
        //         .contains(&Event::MachineBudgetExhausted)
        // );
    }

    #[test]
    fn test_proportion_block_immediate_via_queue() {
        let machine = machine! {
            queues: [None],
            budget: Proportion(0.3),
            state init {},
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        let actions = framework.process(&[], now);
        assert!(actions.is_empty());
        assert_eq!(framework.runtime.decoys_sent, 0);
        assert!(framework.runtime.proportion_blocked);
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached)
        );
    }

    #[test]
    fn test_proportion_allowed_boundary_and_blocking() {
        let machine = machine! {
            queues: [None],
            budget: Proportion(0.5),
            state init {},
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        // send 4 real
        let now = Instant::now();
        for _ in 0..4 {
            let _ = framework.process(&[Event::SendNormal], now);
        }
        assert_eq!(framework.runtime.real_sent, 4);

        // two decoys should be allowed but hit a reached budget
        for _ in 0..2 {
            let _ = framework.runtime.queues[0].push(TimedAction {
                execute_at: now,
                action: IntegratorAction::SendDecoy.into(),
            });
        }

        let actions = framework.process(&[], now);
        assert_eq!(framework.runtime.decoys_sent, 2);
        assert!(
            actions
                .iter()
                .filter(|a| **a == IntegratorAction::SendDecoy)
                .count()
                >= 2
        );
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached)
        );

        // another decoy should not be allowed
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });

        let actions = framework.process(&[], now);
        assert_eq!(actions.len(), 0);
        assert!(framework.runtime.proportion_blocked);
    }

    #[test]
    fn test_machine_exhausted_event_is_emitted_only_once_for_absolute() {
        let machine = machine! {
            queues: [None],
            budget: Absolute(1),
            state init {
                action: IntegratorAction::SendDecoy,
            }
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();
        let _ = framework.process(&[], now);
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetExhausted)
        );

        for _ in 0..3 {
            let _ = framework.process(&[], now);
            assert!(
                !framework
                    .runtime
                    .deferred_events
                    .contains(&Event::MachineBudgetExhausted)
            );
        }
    }

    #[test]
    fn test_proportion_budget_reached_emitted_only_once_when_over_budget() {
        let machine = machine! {
            queues: [None],
            budget: Proportion(0.5),
            state init {},
        }
        .unwrap();
        let mut framework = Framework::new(machine, rand::rng());

        let now = Instant::now();

        framework.process(&[Event::SendNormal; 2], now);
        assert_eq!(framework.runtime.real_sent, 2);
        assert!(!framework.runtime.proportion_blocked);

        // send allowed decoy
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });
        let actions = framework.process(&[], now);
        assert!(actions.contains(&IntegratorAction::SendDecoy));
        assert_eq!(framework.runtime.decoys_sent, 1);
        assert!(framework.runtime.proportion_blocked);

        // drain deferred events
        let _ = framework.process(&[], now);
        assert!(
            !framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached)
        );

        // force reset proportion_blocked and send a decoy
        framework.runtime.proportion_blocked = false;
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });
        let actions = framework.process(&[], now);
        assert!(actions.is_empty());
        assert_eq!(framework.runtime.decoys_sent, 1);
        assert!(framework.runtime.proportion_blocked);
        assert!(
            framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached),
        );

        // drain deferred events
        let _ = framework.process(&[], now);
        assert!(
            !framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached)
        );

        // send another decoy
        let _ = framework.runtime.queues[0].push(TimedAction {
            execute_at: now,
            action: IntegratorAction::SendDecoy.into(),
        });
        let actions = framework.process(&[], now);
        assert!(actions.is_empty());
        assert_eq!(framework.runtime.decoys_sent, 1);
        assert!(
            !framework
                .runtime
                .deferred_events
                .contains(&Event::MachineBudgetReached),
        );
    }
}
