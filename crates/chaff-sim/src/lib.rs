#![expect(dead_code)]
#![expect(missing_docs)]

use chaff::{framework::Framework, trace::Trace};
use rand::Rng;

#[derive(Default)]
pub struct Simulator<R: Rng> {
    framework: Framework<R>,
}

impl<R: Rng> Simulator<R> {
    pub fn run(&self, trace: Trace) -> Trace {
        trace
    }
}

impl<R: Rng> From<Framework<R>> for Simulator<R> {
    fn from(value: Framework<R>) -> Self {
        Self { framework: value }
    }
}
