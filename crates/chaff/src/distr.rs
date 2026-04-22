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
    /// The actual instance of the [`rand_distr`] distribution used for sampling.
    pub distr: ActiveDistr,

    /// Offset added to samples.
    pub offset: Float,

    /// Optional minimum for samples.
    pub min: Option<Float>,

    /// Optional maximum for samples.
    pub max: Option<Float>,
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
        Duration::from_secs_f64(maybe_clamp(
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
        ))
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
