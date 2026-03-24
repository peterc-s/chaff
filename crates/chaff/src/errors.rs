//! Errors for the Chaff library.

// These aren't testable.
#![cfg(not(tarpaulin_include))]

use std::{error::Error, fmt};

/// Primary error type for the `chaff` crate.
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
    TransitionProbsOverOne,
    TransitionToInvalidState,
}

impl Error for ValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransitionProbsOverOne => write!(
                f,
                "transition probabilities sum to a value greater than 1.0"
            ),
            Self::TransitionToInvalidState => write!(f, "transition to an invalid state"),
        }
    }
}
