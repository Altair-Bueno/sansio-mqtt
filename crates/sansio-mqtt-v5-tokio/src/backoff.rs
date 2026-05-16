use std::ops::RangeInclusive;
use std::time::Duration;

/// Configuration for reconnection backoff behaviour.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// The algorithm used to compute the base delay.
    pub algorithm: BackoffAlgorithm,
    /// Inclusive range `[min, max]` that all computed delays are clamped to.
    pub range: RangeInclusive<Duration>,
    /// Seed for the internal xorshift64 pseudo-random number generator.
    /// Deterministic: the same seed produces the same sequence of delays.
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
pub fn compute_delay(backoff: &Backoff, attempt: u32, rng: &mut u64) -> Duration {
    let min = *backoff.range.start();
    let max = *backoff.range.end();

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
