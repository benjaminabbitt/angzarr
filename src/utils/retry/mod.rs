//! Retry utilities with exponential backoff, cap, and jitter.
//!
//! Provides configurable retry logic for saga command execution and other
//! operations that may fail transiently (e.g., sequence conflicts).

use std::time::Duration;
use tonic::{Code, Status};

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Base delay for first retry (before jitter).
    pub base_delay: Duration,
    /// Maximum delay cap (before jitter).
    pub max_delay: Duration,
    /// Maximum number of retry attempts (0 = no retries, just initial attempt).
    pub max_retries: u32,
    /// Jitter factor: delay is multiplied by random value in [1-jitter, 1+jitter].
    /// Set to 0.0 for no jitter.
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(2),
            max_retries: 10,
            jitter: 0.25, // ±25% jitter
        }
    }
}

impl RetryConfig {
    /// Create a retry config for saga command execution.
    ///
    /// Uses sensible defaults for sequence conflict retries:
    /// - Base delay: 10ms
    /// - Max delay: 2s
    /// - Max retries: 10
    /// - Jitter: ±25%
    pub fn for_saga_commands() -> Self {
        Self::default()
    }

    /// Calculate the delay for a given attempt number (0-indexed).
    ///
    /// Uses exponential backoff: delay = base * 2^attempt, capped at max_delay.
    /// Jitter is applied using a simple hash-based approach to avoid thundering herd.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        // Exponential backoff: base * 2^attempt
        let base_ms = self.base_delay.as_millis() as u64;
        let exponential_ms = base_ms.saturating_mul(1u64 << attempt.min(20));

        // Cap at max delay
        let capped_ms = exponential_ms.min(self.max_delay.as_millis() as u64);

        // Apply jitter using simple deterministic hash
        // Uses current time nanos + attempt as entropy source
        let jittered_ms = if self.jitter > 0.0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0) as u64;
            // Simple hash combining time and attempt
            let hash = now.wrapping_mul(31).wrapping_add(attempt as u64 * 17);
            // Map to jitter range: hash % 1000 gives 0-999, normalize to [-jitter, +jitter]
            let jitter_pct = ((hash % 1000) as f64 / 1000.0) * 2.0 - 1.0; // -1.0 to 1.0
            let jitter_factor = 1.0 + (jitter_pct * self.jitter);
            (capped_ms as f64 * jitter_factor) as u64
        } else {
            capped_ms
        };

        Duration::from_millis(jittered_ms)
    }

    /// Check if another retry attempt should be made.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Determines if a gRPC error is retryable (sequence conflict/mismatch).
///
/// Retryable codes:
/// - `FailedPrecondition`: Sequence mismatch (command expected wrong sequence)
/// - `Aborted`: Sequence conflict (concurrent write during persist)
pub fn is_retryable_status(status: &Status) -> bool {
    matches!(status.code(), Code::FailedPrecondition | Code::Aborted)
}

#[cfg(test)]
mod tests;
