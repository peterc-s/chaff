//! Errors for the Chaff library.

// These aren't testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt};

/// Primary error type for the [`crate`] crate.
#[derive(Debug)]
pub enum ChaffError {
    /// Error occurred during validation of a machine spec or its component parts.
    Validation(ValidationError),
}

impl Error for ChaffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for ChaffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => write!(f, "validation error: {e}"),
        }
    }
}

/// Errors that could occur during validation of a machine spec or its component parts.
#[derive(Debug)]
pub enum ValidationError {
    /// [`crate::state::TransitionProbs`] or [`crate::state::Transition`] probabilities exceed `1.0` or are negative.
    BadTransitionProbs(f32),

    /// A [`crate::state::Transition`] leads to an out-of-range/non-existent state.
    TransitionToInvalidState(Box<[usize]>),

    /// a [`crate::state::State`] has an associated [`crate::action::Action`] which would try use a
    /// queue that doesn't exist.
    InvalidStateActionQueue(Box<[u8]>),

    /// Number of queues supplied exceeds the [`u8`] range.
    TooManyQueues(usize),

    /// Multiple [`ValidationError`]s exist.
    Multiple(Box<[Self]>),

    /// A minimum value exceeds a maximum value when it should not.
    MinExceedsMax(String),

    /// A machine must have at least one state.
    NoStates,
}

impl Error for ValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadTransitionProbs(prob) => write!(
                f,
                "sum transition probabilities is either negative or exceeds 1.0 (found: {prob})"
            ),
            Self::TransitionToInvalidState(states) => {
                write!(f, "transition to an invalid state(s) {states:?}")
            }
            Self::InvalidStateActionQueue(queues) => write!(
                f,
                "state(s) contains action(s) which would attempt to use queue(s) that doesn't exist: {queues:?}"
            ),
            Self::TooManyQueues(queues) => {
                write!(f, "number of queues must be <= 255 (got {queues})")
            }
            Self::Multiple(errors) => {
                writeln!(f, "multiple errors:")?;

                for (idx, error) in errors.iter().enumerate() {
                    writeln!(f, "  {}. {}", idx + 1, error)?;
                }

                Ok(())
            }
            Self::MinExceedsMax(err) => write!(f, "minimum exceeds maximum: {err}"),
            Self::NoStates => write!(f, "machines must have at least one state"),
        }
    }
}
