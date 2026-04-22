//! Various disributions from the [`rand_distr`] crate wrapped in a [`Distr`] struct.

// NOTE: this is a bit of a mess but is one of the only ways to really work around avoiding `dyn` or
// a type erased `DynDistr` `trait` which Chaff used to use and was flawed.

use std::time::Duration;

use rand::distr::Distribution;
use rand_distr::{
    Beta, Binomial, Cauchy, Exp, Gamma, Geometric, Hypergeometric, LogNormal, Normal, Pareto,
    Poisson, SkewNormal, Uniform, Weibull,
};

use crate::errors::ValidationError;

type Float = f64;

/// Kinds of supported distributions. See the associated [`rand_distr`] documentation for
/// parameters.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[expect(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistrKind {
    /// A constant value.
    Constant(Float),
    Uniform {
        low: Float,
        high: Float,
    },
    Normal {
        mean: Float,
        std_dev: Float,
    },
    SkewNormal {
        location: Float,
        scale: Float,
        shape: Float,
    },
    Cauchy {
        median: Float,
        scale: Float,
    },
    Binomial {
        n: u64,
        p: f64,
    },
    Geometric {
        p: f64,
    },
    Hypergeometric {
        total_population_size: u64,
        population_with_feature: u64,
        sample_size: u64,
    },
    LogNormal {
        mu: Float,
        sigma: Float,
    },
    Pareto {
        scale: Float,
        shape: Float,
    },
    Poisson {
        lambda: Float,
    },
    Exp {
        lambda: Float,
    },
    Weibull {
        scale: Float,
        shape: Float,
    },
    Gamma {
        shape: Float,
        scale: Float,
    },
    Beta {
        alpha: Float,
        beta: Float,
    },
}

/// Thin wrapper around an instance of a distribution.
#[derive(Clone, Copy, Debug, PartialEq)]
#[expect(missing_docs)]
pub enum ActiveDistr {
    Constant(Float),
    Uniform(Uniform<Float>),
    Normal(Normal<Float>),
    SkewNormal(SkewNormal<Float>),
    Cauchy(Cauchy<Float>),
    Binomial(Binomial),
    Geometric(Geometric),
    Hypergeometric(Hypergeometric),
    LogNormal(LogNormal<Float>),
    Pareto(Pareto<Float>),
    Poisson(Poisson<Float>),
    Exp(Exp<Float>),
    Weibull(Weibull<Float>),
    Gamma(Gamma<Float>),
    Beta(Beta<Float>),
}

impl TryFrom<DistrKind> for ActiveDistr {
    type Error = ValidationError;

    fn try_from(value: DistrKind) -> Result<Self, Self::Error> {
        macro_rules! make_distr {
            ($dist:ident($($arg:expr),*)) => {
                ActiveDistr::$dist($dist::new($($arg),*)
                    .map_err(|e| ValidationError::InvalidDistr(e.to_string()))?)
            };
        }

        Ok(match value {
            DistrKind::Constant(val) => Self::Constant(val),
            DistrKind::Uniform { low, high } => {
                make_distr!(Uniform(low, high))
            }
            DistrKind::Normal { mean, std_dev } => {
                if std_dev < 0.0 {
                    return Err(ValidationError::InvalidDistr(
                        "standard deviation for normal distribution must be non-negative".into(),
                    ));
                }
                make_distr!(Normal(mean, std_dev))
            }
            DistrKind::SkewNormal {
                location,
                scale,
                shape,
            } => {
                make_distr!(SkewNormal(location, scale, shape))
            }
            DistrKind::Cauchy { median, scale } => {
                make_distr!(Cauchy(median, scale))
            }
            DistrKind::Binomial { n, p } => {
                make_distr!(Binomial(n, p))
            }
            DistrKind::Geometric { p } => {
                if p <= 0.0 || p > 1.0 {
                    return Err(ValidationError::InvalidDistr(
                        "geometric parameter p must be in the range (0, 1]".into(),
                    ));
                }
                make_distr!(Geometric(p))
            }
            DistrKind::Hypergeometric {
                total_population_size,
                population_with_feature,
                sample_size,
            } => {
                make_distr!(Hypergeometric(
                    total_population_size,
                    population_with_feature,
                    sample_size
                ))
            }
            DistrKind::LogNormal { mu, sigma } => {
                make_distr!(LogNormal(mu, sigma))
            }
            DistrKind::Pareto { scale, shape } => {
                make_distr!(Pareto(scale, shape))
            }
            DistrKind::Poisson { lambda } => {
                make_distr!(Poisson(lambda))
            }
            DistrKind::Exp { lambda } => {
                make_distr!(Exp(lambda))
            }
            DistrKind::Weibull { scale, shape } => {
                make_distr!(Weibull(scale, shape))
            }
            DistrKind::Gamma { shape, scale } => {
                make_distr!(Gamma(shape, scale))
            }
            DistrKind::Beta { alpha, beta } => {
                make_distr!(Beta(alpha, beta))
            }
        })
    }
}

/// An instance of a distribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distr {
    kind: DistrKind,

    /// The actual instance of the [`rand_distr`] distribution used for sampling.
    pub distr: ActiveDistr,

    /// Offset added to samples.
    pub offset: Float,

    /// Optional minimum for samples.
    pub min: Option<Float>,

    /// Optional maximum for samples.
    pub max: Option<Float>,
}

#[cfg(feature = "borsh")]
impl borsh::BorshSerialize for Distr {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.kind.serialize(writer)?;
        self.offset.serialize(writer)?;
        self.min.serialize(writer)?;
        self.max.serialize(writer)?;
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl borsh::BorshDeserialize for Distr {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let kind = DistrKind::deserialize_reader(reader)?;
        let offset = Float::deserialize_reader(reader)?;
        let min = Option::<Float>::deserialize_reader(reader)?;
        let max = Option::<Float>::deserialize_reader(reader)?;

        let active = ActiveDistr::try_from(kind)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        Ok(Self {
            kind,
            distr: active,
            offset,
            min,
            max,
        })
    }
}

impl Distr {
    /// Try to create a new [`Distr`]. Fails if creating the underlying [`rand_distr`]
    /// [`rand_distr::Distribution`] fails.
    pub fn try_new(
        kind: DistrKind,
        offset: Float,
        min: Option<Float>,
        max: Option<Float>,
    ) -> Result<Self, ValidationError> {
        let distr = ActiveDistr::try_from(kind)?;
        Ok(Self {
            kind,
            distr,
            offset,
            min,
            max,
        })
    }

    /// Sets the [`Distr::offset`].
    #[must_use]
    pub fn with_offset(mut self, offset: Float) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the [`Distr::min`].
    #[must_use]
    pub fn with_min(mut self, min: Option<Float>) -> Self {
        self.min = min;
        self
    }

    /// Sets the [`Distr::max`].
    #[must_use]
    pub fn with_max(mut self, max: Option<Float>) -> Self {
        self.max = max;
        self
    }
}

fn maybe_clamp(val: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    match (min, max) {
        (None, None) => val,
        (None, Some(max)) => val.min(max),
        (Some(min), None) => val.max(min),
        (Some(min), Some(max)) => val.clamp(min, max),
    }
}

impl Distribution<Duration> for Distr {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Duration {
        Duration::from_secs_f64(
            maybe_clamp(
                self.offset
                    + match self.distr {
                        ActiveDistr::Constant(val) => val,
                        ActiveDistr::Uniform(uniform) => uniform.sample(rng),
                        ActiveDistr::Normal(normal) => normal.sample(rng),
                        ActiveDistr::SkewNormal(skew_normal) => skew_normal.sample(rng),
                        ActiveDistr::Cauchy(cauchy) => cauchy.sample(rng),
                        ActiveDistr::Binomial(binomial) => binomial.sample(rng) as f64,
                        ActiveDistr::Geometric(geometric) => geometric.sample(rng) as f64,
                        ActiveDistr::Hypergeometric(hypergeometric) => {
                            hypergeometric.sample(rng) as f64
                        }
                        ActiveDistr::LogNormal(log_normal) => log_normal.sample(rng),
                        ActiveDistr::Pareto(pareto) => pareto.sample(rng),
                        ActiveDistr::Poisson(poisson) => poisson.sample(rng),
                        ActiveDistr::Exp(exp) => exp.sample(rng),
                        ActiveDistr::Weibull(weibull) => weibull.sample(rng),
                        ActiveDistr::Gamma(gamma) => gamma.sample(rng),
                        ActiveDistr::Beta(beta) => beta.sample(rng),
                    },
                self.min,
                self.max,
            )
            .max(0.0),
        )
    }
}

impl TryFrom<DistrKind> for Distr {
    type Error = ValidationError;

    fn try_from(value: DistrKind) -> Result<Self, Self::Error> {
        Self::try_new(value, 0.0, None, None)
    }
}

impl TryFrom<Duration> for Distr {
    type Error = ValidationError;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        Self::try_new(DistrKind::Constant(value.as_secs_f64()), 0.0, None, None)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample(kind: DistrKind) -> f64 {
        let distr = Distr::try_from(kind).unwrap();
        let dur = distr.sample(&mut rand::rng());
        dur.as_secs_f64()
    }

    #[test]
    fn test_all_distr_kinds_construct() {
        let kinds = [
            DistrKind::Constant(1.5),
            DistrKind::Uniform {
                low: 0.0,
                high: 1.0,
            },
            DistrKind::Normal {
                mean: 0.0,
                std_dev: 1.0,
            },
            DistrKind::SkewNormal {
                location: 0.0,
                scale: 1.0,
                shape: 2.0,
            },
            DistrKind::Cauchy {
                median: 0.0,
                scale: 1.0,
            },
            DistrKind::Binomial { n: 10, p: 0.5 },
            DistrKind::Geometric { p: 0.5 },
            DistrKind::Hypergeometric {
                total_population_size: 100,
                population_with_feature: 50,
                sample_size: 10,
            },
            DistrKind::LogNormal {
                mu: 0.0,
                sigma: 1.0,
            },
            DistrKind::Pareto {
                scale: 1.0,
                shape: 2.0,
            },
            DistrKind::Poisson { lambda: 3.0 },
            DistrKind::Exp { lambda: 1.0 },
            DistrKind::Weibull {
                scale: 1.0,
                shape: 1.5,
            },
            DistrKind::Gamma {
                shape: 2.0,
                scale: 1.0,
            },
            DistrKind::Beta {
                alpha: 2.0,
                beta: 5.0,
            },
        ];
        for kind in kinds {
            assert!(
                Distr::try_from(kind).is_ok(),
                "failed to construct {kind:?}"
            );
        }
    }

    #[test]
    fn test_invalid_distr_returns_err() {
        assert!(
            Distr::try_from(DistrKind::Normal {
                mean: 0.0,
                std_dev: -1.0
            })
            .is_err()
        );
        assert!(Distr::try_from(DistrKind::Geometric { p: 0.0 }).is_err());
        assert!(Distr::try_from(DistrKind::Poisson { lambda: -1.0 }).is_err());
    }

    #[test]
    fn test_constant_samples_exact_value() {
        assert_eq!(sample(DistrKind::Constant(2.5)), 2.5);
    }

    #[test]
    fn test_try_from_duration() {
        let dur = Duration::from_secs_f64(3.0);
        let distr = Distr::try_from(dur).unwrap();
        assert_eq!(distr.sample(&mut rand::rng()), dur);
    }

    #[test]
    fn test_with_offset() {
        let distr = Distr::try_from(DistrKind::Constant(1.0))
            .unwrap()
            .with_offset(2.0);

        assert_eq!(distr.offset, 2.0);
        assert_eq!(distr.sample(&mut rand::rng()), Duration::from_secs_f64(3.0));
    }

    #[test]
    fn test_with_min() {
        let distr = Distr::try_from(DistrKind::Constant(0.5))
            .unwrap()
            .with_min(Some(1.0));

        assert_eq!(distr.sample(&mut rand::rng()), Duration::from_secs_f64(1.0));
    }

    #[test]
    fn test_with_max() {
        let distr = Distr::try_from(DistrKind::Constant(10.0))
            .unwrap()
            .with_max(Some(3.0));

        assert_eq!(distr.sample(&mut rand::rng()), Duration::from_secs_f64(3.0));
    }

    #[test]
    fn test_with_min_and_max() {
        let distr = Distr::try_from(DistrKind::Constant(5.0))
            .unwrap()
            .with_min(Some(1.0))
            .with_max(Some(10.0));

        assert_eq!(distr.sample(&mut rand::rng()), Duration::from_secs_f64(5.0));

        let distr_low = Distr::try_from(DistrKind::Constant(0.0))
            .unwrap()
            .with_min(Some(1.0))
            .with_max(Some(10.0));

        assert_eq!(
            distr_low.sample(&mut rand::rng()),
            Duration::from_secs_f64(1.0)
        );

        let distr_high = Distr::try_from(DistrKind::Constant(20.0))
            .unwrap()
            .with_min(Some(1.0))
            .with_max(Some(10.0));

        assert_eq!(
            distr_high.sample(&mut rand::rng()),
            Duration::from_secs_f64(10.0)
        );
    }

    #[test]
    fn test_with_min_none_clears_min() {
        let distr = Distr::try_from(DistrKind::Constant(0.5))
            .unwrap()
            .with_min(Some(1.0))
            .with_min(None);

        assert_eq!(distr.sample(&mut rand::rng()), Duration::from_secs_f64(0.5));
    }

    // sanity checking and potentially flaky, but flaky errors should not be ignored.

    #[test]
    fn test_continuous_distrs_sample_finite() {
        let kinds = [
            DistrKind::Uniform {
                low: 0.0,
                high: 1.0,
            },
            DistrKind::Normal {
                mean: 0.0,
                std_dev: 1.0,
            },
            DistrKind::SkewNormal {
                location: 0.0,
                scale: 1.0,
                shape: 1.0,
            },
            DistrKind::Cauchy {
                median: 0.0,
                scale: 1.0,
            },
            DistrKind::LogNormal {
                mu: 0.0,
                sigma: 0.5,
            },
            DistrKind::Pareto {
                scale: 1.0,
                shape: 2.0,
            },
            DistrKind::Exp { lambda: 1.0 },
            DistrKind::Weibull {
                scale: 1.0,
                shape: 1.5,
            },
            DistrKind::Gamma {
                shape: 2.0,
                scale: 1.0,
            },
            DistrKind::Beta {
                alpha: 2.0,
                beta: 2.0,
            },
        ];
        let mut rng = rand::rng();
        for kind in kinds {
            let distr = Distr::try_from(kind).unwrap();
            for _ in 0..10 {
                let v = distr.sample(&mut rng).as_secs_f64();
                assert!(v.is_finite(), "{kind:?} produced non-finite sample {v}");
            }
        }
    }

    #[test]
    fn test_discrete_distrs_sample_non_negative() {
        let kinds = [
            DistrKind::Binomial { n: 20, p: 0.5 },
            DistrKind::Geometric { p: 0.3 },
            DistrKind::Hypergeometric {
                total_population_size: 50,
                population_with_feature: 20,
                sample_size: 5,
            },
            DistrKind::Poisson { lambda: 4.0 },
        ];
        let mut rng = rand::rng();
        for kind in kinds {
            let distr = Distr::try_from(kind).unwrap();
            for _ in 0..10 {
                let v = distr.sample(&mut rng).as_secs_f64();
                assert!(v >= 0.0, "{kind:?} produced negative sample {v}");
            }
        }
    }
}
