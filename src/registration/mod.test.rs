//! Tests for republish strategies (backoff algorithms).
//!
//! Components periodically re-register to handle startup races and
//! recovery scenarios. Strategies control the timing:
//! - FixedInterval: Constant delay (good for development)
//! - ExponentialBackoff: Increasing delay (good for production)
//!
//! Why this matters: Without re-registration, components that start
//! before the coordinator would be silently ignored. Re-registration
//! ensures eventual discovery regardless of startup order.
//!
//! Key behaviors verified:
//! - FixedInterval returns constant delay
//! - FixedInterval respects max_attempts
//! - ExponentialBackoff doubles delay each attempt
//! - ExponentialBackoff caps at max delay

use super::*;

// ============================================================================
// FixedInterval Tests
// ============================================================================

/// Fixed interval returns constant delay regardless of attempt number.
///
/// Good for development where predictable timing aids debugging.
#[test]
fn test_fixed_interval_constant_delay() {
    let strategy = FixedInterval::new(Duration::from_secs(5));

    assert_eq!(strategy.next_delay(0), Some(Duration::from_secs(5)));
    assert_eq!(strategy.next_delay(1), Some(Duration::from_secs(5)));
    assert_eq!(strategy.next_delay(100), Some(Duration::from_secs(5)));
}

/// Fixed interval stops after max_attempts.
///
/// Useful for bounded retry scenarios (e.g., fail after N attempts).
#[test]
fn test_fixed_interval_with_max_attempts() {
    let strategy = FixedInterval::new(Duration::from_secs(5)).with_max_attempts(3);

    assert_eq!(strategy.next_delay(0), Some(Duration::from_secs(5)));
    assert_eq!(strategy.next_delay(2), Some(Duration::from_secs(5)));
    assert_eq!(strategy.next_delay(3), None);
    assert_eq!(strategy.next_delay(4), None);
}

// ============================================================================
// ExponentialBackoff Tests
// ============================================================================

/// Exponential backoff doubles delay each attempt.
///
/// 1s → 2s → 4s → 8s → ... (with default multiplier=2)
/// Reduces load on coordinator during sustained unavailability.
#[test]
fn test_exponential_backoff_increases() {
    let strategy = ExponentialBackoff::new().with_jitter(false);

    let d0 = strategy.next_delay(0).unwrap();
    let d1 = strategy.next_delay(1).unwrap();
    let d2 = strategy.next_delay(2).unwrap();

    assert_eq!(d0, Duration::from_secs(1));
    assert_eq!(d1, Duration::from_secs(2));
    assert_eq!(d2, Duration::from_secs(4));
}

/// Exponential backoff caps delay at configured maximum.
///
/// Prevents unbounded delays in long-running failure scenarios.
/// Default max: 60s. With 2x multiplier, reaches max after 6 attempts.
#[test]
fn test_exponential_backoff_caps_at_max() {
    let strategy = ExponentialBackoff::new()
        .with_initial(Duration::from_secs(10))
        .with_max(Duration::from_secs(30))
        .with_jitter(false);

    let d0 = strategy.next_delay(0).unwrap();
    let d1 = strategy.next_delay(1).unwrap();
    let d2 = strategy.next_delay(2).unwrap();
    let d10 = strategy.next_delay(10).unwrap();

    assert_eq!(d0, Duration::from_secs(10));
    assert_eq!(d1, Duration::from_secs(20));
    assert_eq!(d2, Duration::from_secs(30)); // Capped
    assert_eq!(d10, Duration::from_secs(30)); // Still capped
}
