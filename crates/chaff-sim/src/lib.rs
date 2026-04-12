//! The Chaff simulator for creating defended traces with machines.
use std::{
    cmp::Ordering,
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

use chaff::{action::IntegratorAction, event::Event, framework::Framework};
use chaff_capture::trace::{Direction, Trace, TraceBuilder, TracePacket};
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
pub struct Simulator<R: Rng> {
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

impl<R: Rng> Simulator<R> {
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

    /// Run the simulation. This instantiates internal queues with the [`Simulator`]s internal
    /// [`Trace`].
    pub fn run(&mut self) -> Trace {
        self.queue = SimulatorQueue::from(self.trace.clone());
        let mut block_state = BlockState::default();
        let mut out_builder = TraceBuilder::default();
        let base_instant = Instant::now();

        loop {
            if let Some(until) = block_state.until
                && self.queue.peek_time().is_none_or(|t| until <= t)
            {
                self.queue.extend(block_state.release(until));
            }

            let Some(event) = self.queue.pop() else { break };
            let sim_now = event.time;

            if event.event == Event::SendNormal && block_state.is_active_at(sim_now) {
                block_state.buffer(event);
                continue;
            }

            if !event.event.is_deferred()
                && let Ok(direction) = Direction::try_from(event.event)
            {
                out_builder.record(direction, sim_now, event.size);
            }

            let sim_instant = base_instant + Duration::from_micros(sim_now);
            let actions = self.framework.process(&[event.event], sim_instant);

            for action in actions {
                match action {
                    IntegratorAction::SendDecoy => {
                        // TODO: size configuration in machines
                        self.queue.push(SimulatorEvent {
                            event: Event::SendDecoy,
                            time: sim_now,
                            size: 512,
                        });
                    }
                    IntegratorAction::BlockOutgoing(duration) => {
                        #[expect(clippy::cast_possible_truncation)]
                        let end_ts =
                            sim_now + duration.sample_dyn(&mut self.rng).as_micros() as u64;
                        block_state.block(end_ts);
                    }
                    IntegratorAction::ReleaseBlock => {
                        self.queue.extend(block_state.release(sim_now));
                    }
                }
            }
        }

        out_builder.build()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::expect_used)]
mod tests {
    use std::{fs, path::PathBuf, rc::Rc};

    use chaff::{
        machine::Machine,
        state::{State, TransitionProbs},
    };
    use rand::{SeedableRng as _, distr::Uniform, rngs::SmallRng};

    use super::*;

    #[test]
    fn test_sim_round_trip() {
        let trace = Trace {
            directions: Box::new([Direction::Send, Direction::Receive, Direction::Send]),
            timing_deltas: Box::new([10, 20, 30]),
            sizes: Box::new([100, 200, 300]),
        };
        let framework = Framework::new(Machine::default(), rand::rng());
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

            let framework = Framework::new(Machine::default(), rand::rng());
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
            TransitionProbs::new([(Event::ReceiveNormal, (1, 1.0).try_into().unwrap())]).unwrap();
        let trans_1_to_2 =
            TransitionProbs::new([(Event::ReceiveNormal, (2, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_0_to_1), IntegratorAction::ReleaseBlock),
                State::new(
                    Some(trans_1_to_2),
                    IntegratorAction::block_outgoing(Duration::from_secs(999)),
                ),
                State::new(None, IntegratorAction::ReleaseBlock),
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
            TransitionProbs::new([(Event::ReceiveNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans), IntegratorAction::ReleaseBlock),
                State::new(
                    None,
                    IntegratorAction::block_outgoing(Duration::from_micros(50)),
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
            TransitionProbs::new([(Event::ReceiveNormal, (1, 1.0).try_into().unwrap())]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans), IntegratorAction::ReleaseBlock),
                State::new(None, IntegratorAction::SendDecoy),
            ],
            0,
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
        let uniform = Uniform::new(Duration::from_micros(110), Duration::from_micros(160))
            .expect("valid uniform range");

        let trans_0_to_1 =
            TransitionProbs::from_tuples([(Event::ReceiveNormal, (1, 1.0))]).unwrap();

        let machine = Machine::new(
            vec![
                State::new(Some(trans_0_to_1), IntegratorAction::ReleaseBlock),
                State::new(None, IntegratorAction::BlockOutgoing(Rc::new(uniform))),
            ],
            0,
        )
        .unwrap();

        let framework = Framework::new(machine, SmallRng::from_seed([0; 32]));

        let input_trace = Trace {
            directions: Box::new([Direction::Receive, Direction::Send, Direction::Send]),
            timing_deltas: Box::new([0, 10, 100]),
            sizes: Box::new([100, 100, 100]),
        };

        let mut sim = Simulator::with(framework, input_trace, SmallRng::from_seed([0; 32]));
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
        let framework = Framework::new(Machine::default(), rand::rng());
        let mut sim = Simulator::with(framework, Trace::default(), rand::rng());

        let out = sim.run();
        assert!(out.directions.is_empty());
    }
}
