//! Chaff actions for integrators to take.

/// An enum with each of the actions Chaff requires an integrator to make.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    /// FIXME: this is a test action.
    #[default]
    SendDecoy,
}
