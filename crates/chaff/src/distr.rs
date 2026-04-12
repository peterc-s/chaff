//! Provides the [`DynDurationDistr`] trait alongside some [`Distribution`] wrappers for random
//! packet delays and decoy sizing.

use std::{any::Any, fmt::Debug, ops::Add, rc::Rc, time::Duration};

use rand::{Rng, RngCore, distr::Distribution};

use crate::errors::ValidationError;

/// Object-safe wrapper around [`Distribution`] for [`Duration`]s.
pub trait DynDurationDistr: Send + Sync {
    /// See [`Distribution<Duration>::sample`].
    fn sample_dyn(&self, rng: &mut dyn RngCore) -> Duration;

    /// Casts to [`Any`].
    fn as_any(&self) -> &dyn Any;

    /// Dynamic equality checking.
    fn dyn_eq(&self, other: &dyn DynDurationDistr) -> bool;
}

impl<D> DynDurationDistr for D
where
    D: Distribution<Duration> + Clone + Send + Sync + PartialEq + Eq + 'static,
{
    fn sample_dyn(&self, mut rng: &mut dyn RngCore) -> Duration {
        Distribution::sample(self, &mut rng)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_eq(&self, other: &dyn DynDurationDistr) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
}

impl Debug for dyn DynDurationDistr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Box<DynDurationDistr>")
    }
}

impl PartialEq for dyn DynDurationDistr {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other)
    }
}

impl Eq for dyn DynDurationDistr {}

/// Helper trait to allow users to pass [`Duration`]s, [`Rc<dyn DynDurationDistr>`]s, or
/// [`Constant<Duration>`]s to certain methods.
pub trait IntoDurationDistr {
    /// Convert into an [`Rc<dyn DynDurationDistr>`].
    fn into_duration_distr(self) -> Rc<dyn DynDurationDistr>;
}

impl IntoDurationDistr for Duration {
    fn into_duration_distr(self) -> Rc<dyn DynDurationDistr> {
        Rc::new(Constant(self))
    }
}

impl IntoDurationDistr for Rc<dyn DynDurationDistr> {
    fn into_duration_distr(self) -> Rc<dyn DynDurationDistr> {
        self
    }
}

impl IntoDurationDistr for Constant<Duration> {
    fn into_duration_distr(self) -> Rc<dyn DynDurationDistr> {
        Rc::new(self)
    }
}

/// A wrapper around a struct that implements [`Distribution<T>`] with an optional offset, minimum,
/// and maximum value (both inclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundedOffsetDistribution<T: Copy, D: Distribution<T>> {
    distr: D,
    offset: Option<T>,
    min: Option<T>,
    max: Option<T>,
}

impl<T, D: Distribution<T>> BoundedOffsetDistribution<T, D>
where
    T: Ord + Copy + Debug,
{
    /// Create a new bounded offset distribution, wrapping a [`Distribution<T>`].
    pub fn new(
        distr: D,
        offset: Option<T>,
        min: Option<T>,
        max: Option<T>,
    ) -> Result<Self, ValidationError> {
        if let (Some(min), Some(max)) = (&min, &max)
            && min > max
        {
            Err(ValidationError::MinExceedsMax(format!(
                "in BoundedOffsetDistribution::new: min = {min:?}, max = {max:?}"
            )))
        } else {
            Ok(Self {
                distr,
                offset,
                min,
                max,
            })
        }
    }
}

impl<T, D> Distribution<T> for BoundedOffsetDistribution<T, D>
where
    D: Distribution<T>,
    T: Ord + Copy + Add<Output = T>,
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> T {
        let mut val = self.distr.sample(rng);

        if let Some(offset) = self.offset {
            val = val + offset;
        }

        match (self.min, self.max) {
            (None, None) => val,
            (Some(min), None) => val.max(min),
            (None, Some(max)) => val.min(max),
            (Some(min), Some(max)) => val.clamp(min, max),
        }
    }
}

/// A "constant" distribution. Allows using a constant value in delays and decoy sizing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constant<T: Copy>(pub T);

impl<T> Distribution<T> for Constant<T>
where
    T: Copy,
{
    fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> T {
        self.0
    }
}
