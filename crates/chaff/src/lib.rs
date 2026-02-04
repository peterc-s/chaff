//! Chaff is an anti Website Fingerprinting (WF) framework.

pub mod errors;
pub mod trace;

#[cfg(feature = "capture")]
pub mod capture;
