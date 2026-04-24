//! The Chaff simulator for creating defended traces with machines.
use std::{
    cmp::Ordering,
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

use chaff::{action::IntegratorAction, event::Event, framework::Framework};
use chaff_capture::trace::{Direction, Trace, TraceBuilder, TracePacket};
use rand::distr::Distribution as _;
use rand::{CryptoRng, Rng};

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

#[expect(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for SimulatorEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&other.time).map(Ordering::reverse)
    }
}

/// Wrapper around a [`BTreeMap`] with [`u64`] keys and [`VecDeque<SimulatorQueue>`] values for
/// using [`Trace`]s with the [`Simulator`].
#[derive(Default, Debug, Clone)]
pub struct SimulatorQueue(BTreeMap<u64, VecDeque<SimulatorEvent>>);

impl SimulatorQueue {
    /// Peek at the earliest time in the [`SimulatorQueue`], if one exists.
    #[must_use]
    pub fn peek_time(&self) -> Option<u64> {
        self.0.keys().next().copied()
    }

    /// Pop the earliest event from the [`SimulatorQueue`], if one exists.
    pub fn pop(&mut self) -> Option<SimulatorEvent> {
        let mut first_entry = self.0.first_entry()?;
        let bucket = first_entry.get_mut();
        let event = bucket.pop_front();
        if bucket.is_empty() {
            first_entry.remove();
        }
        event
    }

    /// Pushes a [`SimulatorEvent`] to the correct point in the [`SimulatorQueue`].
    pub fn push(&mut self, item: SimulatorEvent) {
        self.0.entry(item.time).or_default().push_back(item);
    }
}

impl From<Trace> for SimulatorQueue {
    fn from(value: Trace) -> Self {
        let mut queue = Self::default();
        let mut time_acc = 0u64;

        for TracePacket(dir, delta, size) in &value {
            time_acc += u64::from(delta);
            queue.push(SimulatorEvent {
                event: if dir == Direction::Send {
                    Event::SendNormal
                } else {
                    Event::ReceiveNormal
                },
                time: time_acc,
                size,
            });
        }

        queue
    }
}

impl Extend<SimulatorEvent> for SimulatorQueue {
    fn extend<T: IntoIterator<Item = SimulatorEvent>>(&mut self, iter: T) {
        for item in iter {
            self.push(item);
        }
    }
}

/// An instance of the Chaff simulator.
#[derive(Clone, Debug)]
pub struct Simulator<R: Rng + CryptoRng> {
    /// An instance of the Chaff [`Framework`] to simulate.
    pub framework: Framework<R>,

    /// The trace to simulate on ([`Simulator::queue`] is filled with this on [`Simulator::run`]).
    trace: Trace,

    /// The simulated queues, filled with a [`Trace`].
    queue: SimulatorQueue,

    /// The [`Rng`] used for [`IntegratorAction`]s.
    rng: R,
}

#[derive(Clone, Debug, Default)]
struct BlockState {
    until: Option<u64>,
    buffered: Vec<SimulatorEvent>,
}

impl BlockState {
    fn is_active_at(&self, time: u64) -> bool {
        self.until.map_or_else(|| false, |until| time <= until)
    }

    fn block(&mut self, until: u64) {
        self.until = Some(until);
    }

    fn buffer(&mut self, event: SimulatorEvent) {
        self.buffered.push(event);
    }

    fn release(&mut self, release_time: u64) -> Vec<SimulatorEvent> {
        self.until = None;
        self.buffered
            .drain(..)
            .map(|mut event| {
                event.time = release_time;
                event
            })
            .collect()
    }
}

impl<R: Rng + CryptoRng> Simulator<R> {
    /// Create a [`Simulator`], taking ownership of a [`Framework`] to simulate, with a given
    /// [`Trace`] to run the simulation on.
    pub fn with(framework: Framework<R>, trace: Trace, rng: R) -> Self {
        Self {
            framework,
            trace,
            queue: SimulatorQueue::default(),
            rng,
        }
    }

    /// Replace the trace the simulator will run on.
    pub fn replace_trace(&mut self, trace: Trace) {
        self.trace = trace;
    }

    /// Get the next earliest "event" time (in the sense that something needs to be handled at that
    /// time). Will return [`Some(0)`] if the framework hasn't done its initial process yet.
    fn next_earliest_time(&self, block_state: &BlockState, base_instant: Instant) -> Option<u64> {
        if !self.framework.is_initialised() {
            return Some(0);
        }

        let mut candidate: Option<u64> = self.queue.peek_time();

        // block release
        if let Some(until) = block_state.until {
            candidate = candidate.map_or(Some(until), |curr| Some(curr.min(until)));
        }

        // framework runtime
        if let Some(runtime_inst) = self.framework.peek_soonest_scheduled_instant() {
            let runtime_ts = if runtime_inst <= base_instant {
                0u64
            } else {
                let micros128 = runtime_inst.duration_since(base_instant).as_micros();
                micros128.try_into().unwrap_or(u64::MAX)
            };

            candidate = candidate.map_or(Some(runtime_ts), |curr| Some(curr.min(runtime_ts)));
        }

        candidate
    }

    /// Run the simulation. This instantiates internal queues with the [`Simulator`]s internal
    /// [`Trace`].
    pub fn run(&mut self) -> Trace {
        self.queue = SimulatorQueue::from(self.trace.clone());
        let mut block_state = BlockState::default();
        let mut out_builder = TraceBuilder::default();
        let base_instant = Instant::now();

        while let Some(sim_now) = self.next_earliest_time(&block_state, base_instant) {
            if block_state.until.is_some_and(|until| until <= sim_now) {
                self.queue.extend(block_state.release(sim_now));
            }

            let mut events_now = Vec::new();
            while self.queue.peek_time().is_some_and(|t| t == sim_now) {
                if let Some(event) = self.queue.pop() {
                    events_now.push(event);
                }
            }

            let mut buffered_events = Vec::with_capacity(events_now.len());
            for event in &events_now {
                if event.event == Event::SendNormal && block_state.is_active_at(event.time) {
                    block_state.buffer(event.clone());
                    continue;
                }

                if !event.event.is_deferred()
                    && let Ok(direction) = Direction::try_from(event.event)
                {
                    out_builder.record(direction, sim_now, event.size);
                }

                buffered_events.push(event.event);
            }

            let sim_instant = base_instant + Duration::from_micros(sim_now);
            let actions = self.framework.process(&buffered_events, sim_instant);

            actions.into_iter().for_each(|action| match action {
                IntegratorAction::SendDecoy => {
                    self.queue.push(SimulatorEvent {
                        event: Event::SendDecoy,
                        time: sim_now,
                        size: 512,
                    });
                }
                IntegratorAction::BlockOutgoing(duration) => {
                    #[expect(clippy::cast_possible_truncation)]
                    let end_ts = sim_now + duration.sample(&mut self.rng).as_micros() as u64;
                    block_state.block(end_ts);
                }
                IntegratorAction::ReleaseBlock => {
                    self.queue.extend(block_state.release(sim_now));
                }
            });
        }

        out_builder.build()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::{fs, path::PathBuf, time::Duration};

    use chacha20::ChaCha20Rng;
    use chaff::{
        action::{FrameworkAction, IntegratorAction},
        distr::{Distr, DistrKind},
        machine,
        machine::Machine,
        state::{State, TransitionProbs},
    };
    use rand::SeedableRng as _;

    use super::*;

    #[test]
    fn test_sim_round_trip() {
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([10, 20, 30]),
            sizes: Box::new([100, 200, 300]),
        };
        let machine = machine! {
            queues: [],
            budget: None,
            state init {},
        }
        .unwrap();
        let framework = Framework::new(machine, rand::rng());
        let mut sim: Simulator<_> = Simulator::with(framework, trace.clone(), rand::rng());

        let out_trace = sim.run();

        assert_eq!(trace, out_trace);
    }

    #[test]
    fn test_sim_round_trip_from_trace_files() {
        let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR").to_string() + "/test-traces");

        let mut found_file = false;
        for file in fs::read_dir(&base_path)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("bin")
            })
        {
            found_file = true;
            let mut path = base_path.clone();
            path.push(file);

            let in_trace = Trace::deserialise(&path).unwrap();

            let machine = machine! {
                queues: [],
                budget: None,
                state init {},
            }
            .unwrap();
            let framework = Framework::new(machine, rand::rng());
            let mut sim = Simulator::with(framework, in_trace.clone(), rand::rng());
            let out_trace = sim.run();

            assert_eq!(in_trace, out_trace);
        }

        assert!(found_file);
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
    fn test_block_and_manual_release() {
        let trans_0_to_1 =
            TransitionProbs::try_new([(Event::ReceiveNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();
        let trans_1_to_2 =
            TransitionProbs::try_new([(Event::ReceiveNormal, [(2, 1.0).try_into().unwrap()])])
                .unwrap();

        let long_delay = DistrKind::Constant(999.0).try_into().unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(
                    Some(trans_0_to_1),
                    Some(IntegratorAction::ReleaseBlock),
                    None,
                ),
                State::new(
                    Some(trans_1_to_2),
                    Some(IntegratorAction::block_outgoing(long_delay)),
                    None,
                ),
                State::new(None, Some(IntegratorAction::ReleaseBlock), None),
            ],
            [],
            None,
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
            rand::rng(),
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
            TransitionProbs::try_new([(Event::ReceiveNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let long_delay = Duration::from_micros(50).try_into().unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans), Some(IntegratorAction::ReleaseBlock), None),
                State::new(
                    None,
                    Some(IntegratorAction::block_outgoing(long_delay)),
                    None,
                ),
            ],
            [],
            None,
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
            rand::rng(),
        );

        let out = sim.run();

        assert_eq!(out.directions.len(), 3);
        assert_eq!(out.timing_deltas[0], 0);
        assert_eq!(out.timing_deltas[1], 50); // released after time elapsed.
        assert_eq!(out.timing_deltas[2], 10);
    }

    #[test]
    fn test_send_decoy() {
        let trans =
            TransitionProbs::try_new([(Event::ReceiveNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans), Some(IntegratorAction::ReleaseBlock), None),
                State::new(None, Some(IntegratorAction::SendDecoy), None),
            ],
            [],
            None,
        )
        .unwrap();

        let mut sim = Simulator::with(
            Framework::new(machine, rand::rng()),
            Trace {
                directions: Box::new([Direction::Receive]),
                timing_deltas: Box::new([0]),
                sizes: Box::new([100]),
            },
            rand::rng(),
        );

        let out = sim.run();

        assert_eq!(out.directions.len(), 2);
        assert_eq!(out.timing_deltas[0], 0);
        assert_eq!(out.timing_deltas[1], 0);
        assert_eq!(out.directions[0], Direction::Receive);
        assert_eq!(out.directions[1], Direction::Send);
    }

    /// This test exists mostly because all other tests use [`chaff::distr::Constant`] and the standard
    /// [`rand::rng()`].
    #[test]
    fn test_with_uniform_distribution() {
        let uniform = DistrKind::Uniform {
            low: Duration::from_micros(110).as_secs_f64(),
            high: Duration::from_micros(160).as_secs_f64(),
        }
        .try_into()
        .unwrap();

        let trans_0_to_1 =
            TransitionProbs::from_tuples([(Event::ReceiveNormal, [(1, 1.0)])]).unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(
                    Some(trans_0_to_1),
                    Some(IntegratorAction::ReleaseBlock),
                    None,
                ),
                State::new(None, Some(IntegratorAction::BlockOutgoing(uniform)), None),
            ],
            [],
            None,
        )
        .unwrap();

        let framework = Framework::new(machine, ChaCha20Rng::from_seed([0; 32]));

        let input_trace = Trace {
            directions: Box::new([Direction::Receive, Direction::Send, Direction::Send]),
            timing_deltas: Box::new([0, 10, 100]),
            sizes: Box::new([100, 100, 100]),
        };

        let mut sim = Simulator::with(framework, input_trace, ChaCha20Rng::from_seed([0; 32]));
        let output_trace = sim.run();

        // expected:
        // recv @ 0, trigger random block outgoing
        // send @ random, random block released
        // send @ random, should have also been blocked and then released immediately

        assert_eq!(output_trace.directions.len(), 3);
        assert_eq!(output_trace.directions[0], Direction::Receive);

        let elapsed: u32 = output_trace.timing_deltas.iter().sum();
        assert!(
            elapsed > 110,
            "total time should be extended due to random block."
        );
    }

    #[test]
    fn test_simulator_empty_trace() {
        let machine = machine! {
            queues: [],
            budget: None,
            state init {},
        }
        .unwrap();
        let framework = Framework::new(machine, rand::rng());
        let mut sim = Simulator::with(framework, Trace::default(), rand::rng());

        let out = sim.run();
        assert!(out.directions.is_empty());
    }

    #[test]
    fn test_block_expires_after_trace_ends() {
        let trans =
            TransitionProbs::try_new([(Event::ReceiveNormal, [(1, 1.0).try_into().unwrap()])])
                .unwrap();

        let long_delay = Duration::from_micros(999).try_into().unwrap();

        let machine = Machine::try_new(
            vec![
                State::new(Some(trans), Some(IntegratorAction::ReleaseBlock), None),
                State::new(
                    None,
                    Some(IntegratorAction::block_outgoing(long_delay)),
                    None,
                ),
            ],
            [],
            None,
        )
        .unwrap();

        let mut sim = Simulator::with(
            Framework::new(machine, rand::rng()),
            Trace {
                directions: Box::new([Direction::Receive, Direction::Send]),
                timing_deltas: Box::new([0, 10]),
                sizes: Box::new([100, 100]),
            },
            rand::rng(),
        );

        let out = sim.run();

        assert_eq!(out.directions.len(), 2);
        assert_eq!(out.directions[0], Direction::Receive);
        assert_eq!(out.directions[1], Direction::Send);

        assert_eq!(out.timing_deltas[1], 999);
    }

    #[test]
    fn test_simulator_replace_trace() {
        let machine = machine! {
            queues: [],
            budget: None,
            state dummy {
                action: IntegratorAction::ReleaseBlock,
            }
        }
        .unwrap();

        let trace_0 = Trace {
            directions: Box::new([Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([0, 10]),
            sizes: Box::new([100, 100]),
        };

        let trace_1 = Trace {
            directions: Box::new([Direction::Send, Direction::Receive]),
            timing_deltas: Box::new([10, 0]),
            sizes: Box::new([20, 20]),
        };

        let mut sim = Simulator::with(
            Framework::new(machine, rand::rng()),
            trace_0.clone(),
            rand::rng(),
        );
        assert_eq!(sim.trace, trace_0);

        sim.replace_trace(trace_1.clone());
        assert_eq!(sim.trace, trace_1);
    }

    #[test]
    fn test_simulator_handles_runtime_action_between_packets() {
        // machine should immediately schedule a decoy for 1s in advance
        let delay_secs = Duration::from_secs(1).as_secs_f64();
        let machine = machine! {
            queues: [None],
            budget: None,
            state init {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    DistrKind::Constant(delay_secs).try_into().unwrap()
                ),
            }
        }
        .unwrap();

        let framework = Framework::new(machine, rand::rng());

        // sandwich the scheduled decoy
        let trace = Trace {
            directions: Box::new([Direction::Receive, Direction::Receive]),
            timing_deltas: Box::new([0, 2_000_000]),
            sizes: Box::new([100, 100]),
        };

        let mut sim = Simulator::with(framework, trace, rand::rng());
        let out = sim.run();

        // expected:
        // recv@0s
        // send@1s
        // recv@2s
        assert_eq!(out.directions.len(), 3, "expected decoy");
        assert_eq!(out.directions[0], Direction::Receive);
        assert_eq!(out.directions[1], Direction::Send);
        assert_eq!(out.directions[2], Direction::Receive);

        assert_eq!(out.timing_deltas[0], 0);
        assert_eq!(out.timing_deltas[1], 1_000_000, "expected decoy");
        assert_eq!(out.timing_deltas[2], 1_000_000);
    }

    #[test]
    fn test_zero_delay_runtime_schedule_processed_same_instant() {
        let machine = machine! {
            queues: [None],
            budget: None,
            state init {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    DistrKind::Constant(0.0).try_into().unwrap()
                ),
            }
        }
        .unwrap();

        let framework = Framework::new(machine, rand::rng());

        let trace = Trace {
            directions: Box::new([Direction::Receive, Direction::Receive]),
            timing_deltas: Box::new([0, 1_000_000]),
            sizes: Box::new([100, 100]),
        };

        let mut sim = Simulator::with(framework, trace, rand::rng());
        let out = sim.run();

        // expected:
        // recv@0 (from trace)
        // send@0 (decoy from machine scheduled)
        // recv@1s (from trace)
        assert_eq!(out.directions.len(), 3);
        assert_eq!(out.directions[0], Direction::Receive);
        assert_eq!(out.directions[1], Direction::Send);
        assert_eq!(out.directions[2], Direction::Receive);
        assert_eq!(out.timing_deltas[0], 0);
        assert_eq!(out.timing_deltas[1], 0);
        assert_eq!(out.timing_deltas[2], 1_000_000);
    }

    #[test]
    fn test_runtime_scheduled_at_same_time_as_trace_event() {
        let machine = machine! {
            queues: [None],
            budget: None,
            state init {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    DistrKind::Constant(1.0).try_into().unwrap()
                ),
            }
        }
        .unwrap();

        let framework = Framework::new(machine, rand::rng());

        let trace = Trace {
            directions: Box::new([Direction::Receive, Direction::Receive]),
            timing_deltas: Box::new([0, 1_000_000]),
            sizes: Box::new([100, 100]),
        };

        let mut sim = Simulator::with(framework, trace, rand::rng());
        let out = sim.run();

        // we don't care about order necessarily as the scheduled action and trace should happen at
        // the same time
        assert!(out.directions.len() == 3);
        assert!(out.directions.contains(&Direction::Receive));
        assert!(out.directions.contains(&Direction::Send));
        assert_eq!(out.timing_deltas[1], 1_000_000);
        assert_eq!(out.timing_deltas[2], 0);
    }

    #[test]
    fn test_multiple_runtime_actions_same_instant() {
        let delay: Distr = DistrKind::Constant(1.0).try_into().unwrap();

        let machine = machine! {
            queues: [None; 2],
            budget: None,
            state init {
                action: FrameworkAction::schedule(IntegratorAction::SendDecoy, 0, delay),
                transitions: [Event::ReceiveNormal => schedule_second]
            },
            state schedule_second {
                action: FrameworkAction::schedule(IntegratorAction::SendDecoy, 1, delay),
            },
        }
        .unwrap();

        let framework = Framework::new(machine, rand::rng());
        let trace = Trace {
            directions: Box::new([Direction::Receive, Direction::Receive]),
            timing_deltas: Box::new([0, 2_000_000]),
            sizes: Box::new([100, 100]),
        };

        let mut sim = Simulator::with(framework, trace, rand::rng());
        let out = sim.run();

        // expected:
        // recv@0 (from trace)
        // send@1s (from either init or schedule_second)
        // send@1s (same as above)
        // recv@2s (from trace)
        assert_eq!(out.directions.len(), 4);
        assert_eq!(out.directions[0], Direction::Receive);
        assert_eq!(out.directions[1], Direction::Send);
        assert_eq!(out.directions[2], Direction::Send);
        assert_eq!(out.directions[3], Direction::Receive);
        assert_eq!(out.timing_deltas[0], 0);
        assert_eq!(out.timing_deltas[1], 1_000_000);
        assert_eq!(out.timing_deltas[2], 0);
        assert_eq!(out.timing_deltas[3], 1_000_000);
    }

    #[test]
    fn test_only_runtime_queued() {
        let machine = machine! {
            queues: [None],
            budget: None,
            state init {
                action: FrameworkAction::schedule(
                    IntegratorAction::SendDecoy,
                    0,
                    DistrKind::Constant(1.0).try_into().unwrap()
                ),
            }
        }
        .unwrap();

        let framework = Framework::new(machine, rand::rng());
        let trace = Trace {
            directions: Box::new([]),
            timing_deltas: Box::new([]),
            sizes: Box::new([]),
        };

        let mut sim = Simulator::with(framework, trace, rand::rng());
        let out = sim.run();

        assert_eq!(out.directions.len(), 1);
        assert_eq!(out.directions[0], Direction::Send);
        assert_eq!(out.timing_deltas[0], 1_000_000);
    }

    // TODO: simulator is "perfect" in that it always jumps to the soonest event and an
    // integrator is not in that they don't know the soonest event. need some way to simulate this
    // and also the tests for it.
}
