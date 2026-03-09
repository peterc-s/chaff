//! Test machine used while developing the Chaff framework.

use chaff::framework::{Machine, State};

/// Construct the test machine.
pub fn construct_test_machine() -> Machine {
    Machine::new(vec![State::new(None)])
}
