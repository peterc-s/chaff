//! Chaff is an anti Website Fingerprinting (WF) framework.

pub mod action;
pub mod errors;
pub mod event;
pub mod framework;
pub mod machine;
pub mod state;
pub mod trace;

#[cfg(feature = "capture")]
pub mod capture;
