//! The Chaff simulator for creating defended traces with machines.
use std::{cmp::Ordering, collections::BinaryHeap, time::Instant};

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

    /// If blocking, when to unblock.
    blocking_until: Option<Instant>,

    /// Events that are currently blocked.
    blocked_events: Vec<SimulatorEvent>,
}

impl<R: Rng> Simulator<R> {
    /// Create a [`Simulator`], taking ownership of a [`Framework`] to simulate, with a given
    /// [`Trace`] to run the simulation on.
    pub fn with(framework: Framework<R>, trace: Trace) -> Self {
        Self {
            framework,
            trace,
            queue: SimulatorQueue::default(),
            blocking_until: None,
            blocked_events: vec![],
        }
    }

    /// Run the simulation. This instantiates internal queues with the [`Simulator::trace`].
    pub fn run(&mut self) -> Trace {
        let trace_len = self.trace.len();
        self.queue = SimulatorQueue::from(self.trace.clone());

        // for output trace
        let mut directions: Vec<Direction> = Vec::with_capacity(trace_len);
        let mut timing_deltas = Vec::with_capacity(trace_len);
        let mut sizes = Vec::with_capacity(trace_len);

        let mut last_event_ts = 0;
        while let Some(event) = self.queue.pop_soonest() {
            let now = Instant::now();

            if event.event == Event::SendNormal {
                if let Some(block_time) = self.blocking_until {
                    if now < block_time {
                        self.blocked_events.push(event);
                        continue;
                    }
                    self.blocking_until = None;
                }
            }

            let actions = self.framework.process(&[event.event], now);

            match event.event {
                Event::SendNormal => directions.push(Direction::Send),
                Event::ReceiveNormal => directions.push(Direction::Receive),
                Event::QueuePopped(_) => {}
            }
            timing_deltas.push(event.time - last_event_ts);
            sizes.push(event.size);
            last_event_ts = event.time;

            for action in actions {
                match action {
                    IntegratorAction::SendDecoy => {
                        if let Some(block_time) = self.blocking_until {
                            if now < block_time {
                                self.blocked_events.push(SimulatorEvent {
                                    event: Event::SendNormal,
                                    time: last_event_ts,
                                    size: 512,
                                });
                            } else {
                                self.blocking_until = None;
                                directions.push(Direction::Send);
                            }
                        }
                    }
                    IntegratorAction::BlockOutgoing(duration) => {
                        self.blocking_until = Some(now + duration);
                    }
                    IntegratorAction::ReleaseBlock => {
                        self.blocking_until = None;
                        while let Some(mut blocked_event) = self.blocked_events.pop() {
                            blocked_event.time = last_event_ts;
                            self.queue.push(blocked_event);
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
    use chaff::machine::Machine;

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
}
