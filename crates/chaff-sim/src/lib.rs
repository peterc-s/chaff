//! The Chaff simulator for creating defended traces with machines.
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    time::{Duration, Instant},
};

use chaff::{action::IntegratorAction, event::Event, framework::Framework};
use chaff_capture::trace::{Direction, Trace, TracePacket};
use rand::Rng;

/// A simulated event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulatorEvent {
    /// Inner [`Event`].
    event: Event,

    /// The time of the [`Event`].
    time: u64,

    /// Size of packet if needed.
    size: u32,
}

impl Ord for SimulatorEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time).reverse()
    }
}

// Since we're reversing the order for BinaryHeap
#[expect(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for SimulatorEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&other.time).map(Ordering::reverse)
    }
}

/// Represents the device's ingress and egress queues.
#[derive(Clone, Debug, Default)]
pub struct SimulatorQueue {
    ingress: BinaryHeap<SimulatorEvent>,
    egress: BinaryHeap<SimulatorEvent>,
}

impl SimulatorQueue {
    /// Pops the soonest event in either the ingress or egress queue.
    pub fn pop_soonest(&mut self) -> Option<SimulatorEvent> {
        match (self.ingress.peek(), self.egress.peek()) {
            (None, None) => None,
            (None, Some(_)) => self.egress.pop(),
            (Some(_), None) => self.ingress.pop(),
            (Some(ingress), Some(egress)) => {
                if ingress.time < egress.time {
                    self.ingress.pop()
                } else {
                    self.egress.pop()
                }
            }
        }
    }

    /// Push an event to the [`SimulatorQueue`].
    ///
    /// [`Event::SendNormal`]s go on the egress queue.
    /// [`Event::ReceiveNormal`]s go on the ingress queue.
    pub fn push(&mut self, event: SimulatorEvent) {
        match event.event {
            Event::SendNormal => self.egress.push(event),
            Event::ReceiveNormal => self.ingress.push(event),
            Event::QueuePopped(_) => {}
        }
    }
}

impl From<Trace> for SimulatorQueue {
    fn from(value: Trace) -> Self {
        let mut ingress = BinaryHeap::new();
        let mut egress = BinaryHeap::new();
        let mut time_acc = 0u64;

        for TracePacket(dir, delta, size) in &value {
            time_acc += u64::from(delta);
            match dir {
                Direction::Send => egress.push(SimulatorEvent {
                    event: Event::SendNormal,
                    time: time_acc,
                    size,
                }),
                Direction::Receive => ingress.push(SimulatorEvent {
                    event: Event::ReceiveNormal,
                    time: time_acc,
                    size,
                }),
            }
        }

        Self { ingress, egress }
    }
}

/// An instance of the Chaff simulator.
#[derive(Clone, Debug)]
pub struct Simulator<R: Rng> {
    /// An instance of the Chaff [`Framework`] to simulate.
    pub framework: Framework<R>,

    /// The trace to simulate on ([`Simulator::queue`] is filled with this on [`Simulator::run`]).
    trace: Trace,

    /// The simulated queues, filled with a [`Trace`].
    queue: SimulatorQueue,
}

impl<R: Rng> Simulator<R> {
    /// Create a [`Simulator`], taking ownership of a [`Framework`] to simulate, with a given
    /// [`Trace`] to run the simulation on.
    pub fn with(framework: Framework<R>, trace: Trace) -> Self {
        Self {
            framework,
            trace,
            queue: SimulatorQueue::default(),
        }
    }

    /// Run the simulation. This instantiates internal queues with the [`Simulator::trace`].
    pub fn run(&mut self) -> Trace {
        let trace_len = self.trace.len();
        self.queue = SimulatorQueue::from(self.trace.clone());
        let mut blocking_until: Option<u64> = None;
        let mut blocked_events: Vec<SimulatorEvent> = vec![];

        // for output trace
        let mut directions: Vec<Direction> = Vec::with_capacity(trace_len);
        let mut timing_deltas = Vec::with_capacity(trace_len);
        let mut sizes = Vec::with_capacity(trace_len);

        let base_instant = Instant::now();
        let mut last_event_ts = 0;

        while let Some(event) = self.queue.pop_soonest() {
            let sim_now = event.time;

            if let Some(until) = blocking_until {
                if sim_now >= until {
                    blocking_until = None;
                    while let Some(mut blocked) = blocked_events.pop() {
                        blocked.time = sim_now;
                        self.queue.push(blocked);
                    }
                }
            }

            if event.event == Event::SendNormal && blocking_until.is_some() {
                blocked_events.push(event);
                continue;
            }

            let sim_instant = base_instant + Duration::from_micros(sim_now);
            let actions = self.framework.process(&[event.event], sim_instant);

            if !matches!(event.event, Event::QueuePopped(_)) {
                directions.push(if event.event == Event::SendNormal {
                    Direction::Send
                } else {
                    Direction::Receive
                });
                timing_deltas.push(sim_now - last_event_ts);
                sizes.push(event.size);
                last_event_ts = sim_now;
            }

            for action in actions {
                match action {
                    IntegratorAction::SendDecoy => {
                        self.queue.push(SimulatorEvent {
                            event: Event::SendNormal,
                            time: sim_now,
                            size: 512,
                        });
                    }
                    IntegratorAction::BlockOutgoing(duration) => {
                        // rust duration micros are u128. change sim to u128 if
                        // this becomes an issue.
                        #[expect(clippy::cast_possible_truncation)]
                        let end_ts = sim_now + duration.as_micros() as u64;
                        blocking_until = Some(end_ts);
                    }
                    IntegratorAction::ReleaseBlock => {
                        blocking_until = None;
                        while let Some(mut blocked) = blocked_events.pop() {
                            blocked.time = sim_now;
                            self.queue.push(blocked);
                        }
                    }
                }
            }
        }

        // event.time is u64 because it is absolute, the deltas are differences and
        // should fall in the trace u32. if this becomes a problem, this may change.
        #[expect(clippy::cast_possible_truncation)]
        Trace {
            directions: directions.into(),
            timing_deltas: timing_deltas.iter().map(|val| *val as u32).collect(),
            sizes: sizes.into(),
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use chaff::{
        machine::Machine,
        state::{State, TransitionProbs},
    };

    use super::*;

    #[test]
    fn test_sim_round_trip() {
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([10, 20, 30]),
            sizes: Box::new([100, 200, 300]),
        };
        let framework = Framework::new(Machine::default(), rand::rng());
        let mut sim: Simulator<_> = Simulator::with(framework, trace.clone());

        let out_trace = sim.run();

        assert_eq!(trace, out_trace);
    }

    #[test]
    fn test_sim_event_ord() {
        let early = SimulatorEvent {
            event: Event::SendNormal,
            time: 10,
            size: 100,
        };
        let late = SimulatorEvent {
            event: Event::SendNormal,
            time: 20,
            size: 100,
        };

        assert_eq!(early.cmp(&late), Ordering::Greater);
        assert_eq!(late.cmp(&early), Ordering::Less);
        assert!(early > late);

        let early_alt = SimulatorEvent {
            event: Event::SendNormal,
            time: 10,
            size: 500,
        };
        assert_eq!(early.time.cmp(&early_alt.time), Ordering::Equal);
    }

    #[test]
    fn test_pop_soonest_ingress() {
        let mut queue = SimulatorQueue::default();

        queue.ingress.push(SimulatorEvent {
            event: Event::ReceiveNormal,
            time: 50,
            size: 0,
        });

        let popped = queue.pop_soonest();

        assert!(popped.is_some());
        assert_eq!(popped.unwrap().event, Event::ReceiveNormal);
        assert!(queue.pop_soonest().is_none());
    }

    #[test]
    fn test_block_and_manual_release() {
        let trans_0_to_1 =
            TransitionProbs::new([(Event::ReceiveNormal, (1, 1.0).try_into().unwrap())]).unwrap();
        let trans_1_to_2 =
            TransitionProbs::new([(Event::ReceiveNormal, (2, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_0_to_1), IntegratorAction::ReleaseBlock.into()),
                State::new(
                    Some(trans_1_to_2),
                    IntegratorAction::BlockOutgoing(Duration::from_secs(999)).into(),
                ),
                State::new(None, IntegratorAction::ReleaseBlock.into()),
            ],
            0,
        )
        .unwrap();

        let mut sim = Simulator::with(
            Framework::new(machine, rand::rng()),
            Trace {
                directions: Box::new([
                    Direction::Receive, // 0: trigger block
                    Direction::Send,    // 1: blocked
                    Direction::Send,    // 2: blocked
                    Direction::Receive, // 3: trigger release
                ]),
                timing_deltas: Box::new([0, 10, 10, 10]),
                sizes: Box::new([100, 100, 100, 100]),
            },
        );

        let out = sim.run();

        // expected:
        // recv @ 10 (recorded)
        // send @ 20 (blocked)
        // send @ 30 (blocked)
        // receive @ 40 (recorded, release)
        // send @ 40 (recorded)
        // send @ 40 (recorded)

        // check expected directions
        assert_eq!(out.directions.len(), 4);
        assert_eq!(out.directions[0], Direction::Receive);
        assert_eq!(out.directions[1], Direction::Receive);
        assert_eq!(out.directions[2], Direction::Send);
        assert_eq!(out.directions[3], Direction::Send);

        // check final burst timings
        assert_eq!(out.timing_deltas[2], 0);
        assert_eq!(out.timing_deltas[3], 0);
    }

    #[test]
    fn test_block_natural_expiration() {
        let trans =
            TransitionProbs::new([(Event::ReceiveNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans), IntegratorAction::ReleaseBlock.into()),
                State::new(
                    None,
                    IntegratorAction::BlockOutgoing(Duration::from_micros(50)).into(),
                ),
            ],
            0,
        )
        .unwrap();

        let mut sim = Simulator::with(
            Framework::new(machine, rand::rng()),
            Trace {
                directions: Box::new([
                    Direction::Receive, // 0: block until 50
                    Direction::Send,    // 10: should be blocked
                    Direction::Send,    // 60: should not be blocked
                ]),
                timing_deltas: Box::new([0, 10, 50]),
                sizes: Box::new([100, 100, 100]),
            },
        );

        let out = sim.run();

        assert_eq!(out.directions.len(), 3);
        assert_eq!(out.timing_deltas[0], 0);
        assert_eq!(out.timing_deltas[1], 50); // released after time elapsed.
        assert_eq!(out.timing_deltas[2], 60);
    }
}
