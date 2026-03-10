#![expect(missing_docs)]

use std::{cmp::Ordering, collections::BinaryHeap};

use chaff::{
    framework::{Event, Framework},
    trace::{Direction, Trace, TracePacket},
};
use rand::Rng;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulatorEvent {
    event: Event,
    time: u64,
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

#[derive(Clone, Debug, Default)]
pub struct SimulatorQueue {
    ingress: BinaryHeap<SimulatorEvent>,
    egress: BinaryHeap<SimulatorEvent>,
}

impl SimulatorQueue {
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

#[derive(Clone, Debug)]
pub struct Simulator<R: Rng> {
    pub framework: Framework<R>,
    trace: Trace,
    queue: SimulatorQueue,
}

impl<R: Rng> Simulator<R> {
    pub fn with(framework: Framework<R>, trace: Trace) -> Self {
        Self {
            framework,
            trace,
            queue: SimulatorQueue::default(),
        }
    }

    pub fn run(&mut self) -> Trace {
        let trace_len = self.trace.len();
        self.queue = SimulatorQueue::from(self.trace.clone());

        // for output trace
        let mut directions = Vec::with_capacity(trace_len);
        let mut timing_deltas = Vec::with_capacity(trace_len);
        let mut sizes = Vec::with_capacity(trace_len);

        let mut last_event_ts = 0;
        while let Some(event) = self.queue.pop_soonest() {
            // unused for now
            let _ = self.framework.trigger_events(&[event.event]);

            directions.push(match event.event {
                Event::SendNormal => Direction::Send,
                Event::ReceiveNormal => Direction::Receive,
            });
            timing_deltas.push(event.time - last_event_ts);
            sizes.push(event.size);

            last_event_ts = event.time;
        }

        // event.time is u64 because it is absolute, the deltas are differences and
        // should fall in the trace u32.
        // TODO: if this becomes a problem, handle it properly.
        #[expect(clippy::cast_possible_truncation)]
        Trace {
            directions: directions.into(),
            timing_deltas: timing_deltas.iter().map(|val| *val as u32).collect(),
            sizes: sizes.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use chaff::framework::Machine;

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
}
