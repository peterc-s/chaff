#![expect(dead_code)]
#![expect(missing_docs)]

use chaff::{framework::Framework, trace::Trace};

#[derive(Default)]
pub struct Simulator {
    framework: Framework,
}

impl Simulator {
    pub fn run(&self, trace: Trace) -> Trace {
        trace
    }
}

impl From<Framework> for Simulator {
    fn from(value: Framework) -> Self {
        Self { framework: value }
    }
}
