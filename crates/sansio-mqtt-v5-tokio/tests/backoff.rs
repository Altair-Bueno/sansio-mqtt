use sansio_mqtt_v5_tokio::backoff::compute_delay;
use sansio_mqtt_v5_tokio::backoff::Backoff;
use sansio_mqtt_v5_tokio::backoff::BackoffAlgorithm;
use std::time::Duration;

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
    // 1 * 2^0 = 1s
    assert_eq!(compute_delay(&b, 0, &mut rng), Duration::from_secs(1));
    // 1 * 2^3 = 8s
    assert_eq!(compute_delay(&b, 3, &mut rng), Duration::from_secs(8));
    // 1 * 2^100 >> 60s — clamped
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
