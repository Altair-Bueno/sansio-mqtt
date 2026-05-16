use std::ops::RangeInclusive;
use std::time::Duration;

/// Configuration for reconnection backoff behaviour.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// The algorithm used to compute the base delay.
    pub algorithm: BackoffAlgorithm,
    /// Inclusive range `[min, max]` that all computed delays are clamped to.
    ///
    /// `range.start()` must be ≤ `range.end()`. If `range.start() >
    /// range.end()` the behaviour is unspecified (currently returns
    /// `range.start()`).
    pub range: RangeInclusive<Duration>,
    /// Seed for the internal xorshift64 pseudo-random number generator.
    ///
    /// Deterministic: the same seed produces the same sequence of delays.
    /// A seed of `0` is normalised to `1` internally because xorshift64
    /// requires a non-zero state.
    pub seed: u64,
}

/// Algorithm used to compute the reconnection delay for each attempt.
#[derive(Debug, Clone)]
pub enum BackoffAlgorithm {
    /// `delay = range.start + slope * attempt`, clamped to `range.end`.
    Linear { slope: Duration },
    /// `delay = range.start * factor^attempt`, clamped to `range.end`.
    Exponential { factor: f64 },
    /// Uniform random delay in `range.start..=range.end`, independent of
    /// attempt.
    Jitter,
    /// Exponential base delay with uniform random jitter in `0..=base`, clamped
    /// to `range.end`.
    JitteredExponential { factor: f64 },
}

/// Advance the xorshift64 PRNG and return the next value.
///
/// The state must never be zero; callers are responsible for initialising it
/// with a non-zero seed.  A seed of `0` will always produce `0`.
fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Compute the reconnection delay for the given `attempt` (0-indexed).
///
/// `rng` is the caller-maintained PRNG state; it is mutated only for
/// algorithms that require randomness ([`BackoffAlgorithm::Jitter`] and
/// [`BackoffAlgorithm::JitteredExponential`]).
pub(crate) fn compute_delay(backoff: &Backoff, attempt: u32, rng: &mut u64) -> Duration {
    let min = *backoff.range.start();
    let max = *backoff.range.end();

    if min > max {
        return min;
    }

    let raw = match backoff.algorithm {
        BackoffAlgorithm::Linear { slope } => min.saturating_add(slope.saturating_mul(attempt)),
        BackoffAlgorithm::Exponential { factor } => {
            let secs = min.as_secs_f64() * factor.powi(attempt as i32);
            Duration::from_secs_f64(secs.min(max.as_secs_f64()))
        }
        BackoffAlgorithm::Jitter => {
            let range_nanos = (max - min).as_nanos() as u64;
            let offset = if range_nanos == 0 {
                0
            } else {
                xorshift64(rng) % range_nanos
            };
            min.saturating_add(Duration::from_nanos(offset))
        }
        BackoffAlgorithm::JitteredExponential { factor } => {
            let exp_secs = min.as_secs_f64() * factor.powi(attempt as i32);
            let exp = Duration::from_secs_f64(exp_secs.min(max.as_secs_f64()));
            let exp_nanos = exp.as_nanos() as u64;
            let offset = if exp_nanos == 0 {
                0
            } else {
                xorshift64(rng) % exp_nanos
            };
            exp.saturating_add(Duration::from_nanos(offset))
        }
    };

    raw.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_first_delay_equals_range_start() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Linear {
                slope: Duration::from_secs(10),
            },
            range: Duration::from_secs(5)..=Duration::from_secs(60),
            seed: 0,
        };
        let mut rng = 1u64;
        assert_eq!(compute_delay(&b, 0, &mut rng), Duration::from_secs(5));
    }

    #[test]
    fn linear_grows_by_slope_per_attempt() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Linear {
                slope: Duration::from_secs(10),
            },
            range: Duration::from_secs(5)..=Duration::from_secs(60),
            seed: 0,
        };
        let mut rng = 1u64;
        assert_eq!(compute_delay(&b, 1, &mut rng), Duration::from_secs(15));
        assert_eq!(compute_delay(&b, 2, &mut rng), Duration::from_secs(25));
    }

    #[test]
    fn linear_clamps_at_range_end() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Linear {
                slope: Duration::from_secs(10),
            },
            range: Duration::from_secs(5)..=Duration::from_secs(60),
            seed: 0,
        };
        let mut rng = 1u64;
        assert_eq!(compute_delay(&b, 100, &mut rng), Duration::from_secs(60));
    }

    #[test]
    fn exponential_clamps_at_range_end() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Exponential { factor: 2.0 },
            range: Duration::from_secs(1)..=Duration::from_secs(60),
            seed: 0,
        };
        let mut rng = 1u64;
        assert_eq!(compute_delay(&b, 0, &mut rng), Duration::from_secs(1));
        assert_eq!(compute_delay(&b, 3, &mut rng), Duration::from_secs(8));
        assert_eq!(compute_delay(&b, 100, &mut rng), Duration::from_secs(60));
    }

    #[test]
    fn jitter_stays_within_range() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Jitter,
            range: Duration::from_secs(5)..=Duration::from_secs(60),
            seed: 42,
        };
        let mut rng = 42u64;
        for _ in 0..1000 {
            let d = compute_delay(&b, 0, &mut rng);
            assert!(
                d >= Duration::from_secs(5),
                "jitter below range.start: {d:?}"
            );
            assert!(
                d <= Duration::from_secs(60),
                "jitter above range.end: {d:?}"
            );
        }
    }

    #[test]
    fn jitter_is_deterministic_given_same_seed() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Jitter,
            range: Duration::from_secs(1)..=Duration::from_secs(30),
            seed: 99,
        };
        let mut rng_a = 99u64;
        let mut rng_b = 99u64;
        for _ in 0..20 {
            assert_eq!(
                compute_delay(&b, 0, &mut rng_a),
                compute_delay(&b, 0, &mut rng_b)
            );
        }
    }

    #[test]
    fn jittered_exponential_stays_within_range() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::JitteredExponential { factor: 2.0 },
            range: Duration::from_secs(1)..=Duration::from_secs(60),
            seed: 7,
        };
        let mut rng = 7u64;
        for attempt in 0..20u32 {
            let d = compute_delay(&b, attempt, &mut rng);
            assert!(
                d >= Duration::from_secs(1),
                "below range.start at attempt {attempt}: {d:?}"
            );
            assert!(
                d <= Duration::from_secs(60),
                "above range.end at attempt {attempt}: {d:?}"
            );
        }
    }

    #[test]
    fn inverted_range_does_not_panic() {
        let b = Backoff {
            algorithm: BackoffAlgorithm::Jitter,
            range: Duration::from_secs(10)..=Duration::from_secs(1),
            seed: 1,
        };
        let mut rng = 1u64;
        // Should return range.start without panicking.
        assert_eq!(compute_delay(&b, 0, &mut rng), Duration::from_secs(10));
    }
}
