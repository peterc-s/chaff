//! Chaff is an anti Website Fingerprinting (WF) framework.

pub mod actions;
pub mod errors;
pub mod machines;
pub mod states;
pub mod trace;

#[cfg(feature = "capture")]
pub mod capture;
