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
        write!(f, "DynDurationDistr")
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
    // tarpaulin can't seem to pick this up, but it is tested and trivial.
    #[cfg(not(tarpaulin_include))]
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
    ///
    /// # Errors
    ///
    /// Will throw a [`ValidationError::MinExceedsMax`] if the `min` argument exceeds the `max` argument.
    pub fn new(
        distr: D,
        offset: Option<T>,
        min: Option<T>,
        max: Option<T>,
    ) -> Result<Self, ValidationError> {
        if let (Some(min), Some(max)) = (min, max)
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

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_dyn_duration_distr_eq() {
        let d1: Rc<dyn DynDurationDistr> = Rc::new(Constant(Duration::from_secs(1)));
        let d2: Rc<dyn DynDurationDistr> = Rc::new(Constant(Duration::from_secs(1)));
        let d3: Rc<dyn DynDurationDistr> = Rc::new(Constant(Duration::from_secs(2)));

        // TODO: figure out if it's possible to use assert_eq and assert_ne instead.
        assert!(d1 == Rc::clone(&d2));
        assert!(d2 != d3);
    }

    #[test]
    fn test_dyn_duration_debug() {
        let d: Rc<dyn DynDurationDistr> = Rc::new(Constant(Duration::from_secs(1)));
        assert_eq!(format!("{d:?}"), "DynDurationDistr");
    }

    #[test]
    fn test_into_duration_distr() {
        let mut rng = rand::rng();
        let base = Duration::from_secs(1);
        assert_eq!(base.into_duration_distr().sample_dyn(&mut rng), base);
        assert_eq!(
            Constant(base).into_duration_distr().sample_dyn(&mut rng),
            base
        );
        assert_eq!(
            Rc::new(Constant(base))
                .into_duration_distr()
                .sample_dyn(&mut rng),
            base
        );
    }

    #[test]
    fn test_bounded_offset_min_exceed_max() {
        let err = BoundedOffsetDistribution::new(
            Constant(Duration::from_secs(1)),
            None,
            Some(Duration::from_secs(10)),
            Some(Duration::from_secs(5)),
        )
        .unwrap_err();

        assert!(matches!(err, ValidationError::MinExceedsMax(_)));
    }

    #[test]
    fn test_bounded_offset_distribution_sampling() {
        let mut rng = rand::rng();
        let base = Constant(Duration::from_secs(10));

        let dist1 =
            BoundedOffsetDistribution::new(base, Some(Duration::from_secs(2)), None, None).unwrap();
        assert_eq!(dist1.sample(&mut rng), Duration::from_secs(12));

        let dist2 = BoundedOffsetDistribution::new(base, None, Some(Duration::from_secs(15)), None)
            .unwrap();
        assert_eq!(dist2.sample(&mut rng), Duration::from_secs(15));

        let dist3 =
            BoundedOffsetDistribution::new(base, None, None, Some(Duration::from_secs(5))).unwrap();
        assert_eq!(dist3.sample(&mut rng), Duration::from_secs(5));

        let dist4 = BoundedOffsetDistribution::new(
            base,
            None,
            Some(Duration::from_secs(1)),
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        assert_eq!(dist4.sample(&mut rng), Duration::from_secs(5));
    }
}
