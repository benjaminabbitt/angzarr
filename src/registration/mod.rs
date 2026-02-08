//! Component registration and republish strategies.
//!
//! Provides configurable strategies for how often components re-register
//! with the topology projector to handle startup races.

use std::time::Duration;

/// Strategy for determining when to re-register components.
///
/// Implementations return the delay before the next registration attempt,
/// or `None` to stop re-registering.
pub trait RepublishStrategy: Send + Sync {
    /// Returns the delay before the next republish, or None to stop.
    fn next_delay(&self, attempt: u32) -> Option<Duration>;
}

/// Fixed interval republish strategy.
///
/// Republishes at a constant rate with no backoff. Good for development
/// where fast feedback is preferred.
#[derive(Debug, Clone)]
pub struct FixedInterval {
    interval: Duration,
    max_attempts: Option<u32>,
}

impl FixedInterval {
    /// Create a fixed interval strategy.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            max_attempts: None,
        }
    }

    /// Limit the number of republish attempts.
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = Some(max);
        self
    }
}

impl RepublishStrategy for FixedInterval {
    fn next_delay(&self, attempt: u32) -> Option<Duration> {
        if let Some(max) = self.max_attempts {
            if attempt >= max {
                return None;
            }
        }
        Some(self.interval)
    }
}

/// Exponential backoff republish strategy.
///
/// Starts with frequent republishes, then backs off exponentially.
/// Good for production where we want to handle startup races but
/// reduce load over time.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    initial: Duration,
    max: Duration,
    multiplier: f64,
    jitter: bool,
}

impl ExponentialBackoff {
    /// Create an exponential backoff strategy with sensible defaults.
    ///
    /// Defaults: initial=1s, max=60s, multiplier=2.0, jitter=true
    pub fn new() -> Self {
        Self {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Set the initial delay.
    pub fn with_initial(mut self, initial: Duration) -> Self {
        self.initial = initial;
        self
    }

    /// Set the maximum delay.
    pub fn with_max(mut self, max: Duration) -> Self {
        self.max = max;
        self
    }

    /// Set the backoff multiplier.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Enable or disable jitter.
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new()
    }
}

impl RepublishStrategy for ExponentialBackoff {
    fn next_delay(&self, attempt: u32) -> Option<Duration> {
        let base_delay = self.initial.as_secs_f64() * self.multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max.as_secs_f64());

        let final_delay = if self.jitter {
            // Add up to 25% jitter using nanosecond timestamp as entropy source
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let jitter_factor = 1.0 + ((nanos % 250) as f64 / 1000.0); // 0-25%
            capped_delay * jitter_factor
        } else {
            capped_delay
        };

        Some(Duration::from_secs_f64(final_delay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_interval_constant_delay() {
        let strategy = FixedInterval::new(Duration::from_secs(5));

        assert_eq!(strategy.next_delay(0), Some(Duration::from_secs(5)));
        assert_eq!(strategy.next_delay(1), Some(Duration::from_secs(5)));
        assert_eq!(strategy.next_delay(100), Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_fixed_interval_with_max_attempts() {
        let strategy = FixedInterval::new(Duration::from_secs(5)).with_max_attempts(3);

        assert_eq!(strategy.next_delay(0), Some(Duration::from_secs(5)));
        assert_eq!(strategy.next_delay(2), Some(Duration::from_secs(5)));
        assert_eq!(strategy.next_delay(3), None);
        assert_eq!(strategy.next_delay(4), None);
    }

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
}
