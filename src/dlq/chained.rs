//! Chained DLQ publisher for priority list fallback.
//!
//! Tries each target in order until one succeeds.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use super::error::DlqError;
use super::{AngzarrDeadLetter, DeadLetterPublisher};

/// Chained DLQ publisher that tries targets in priority order.
///
/// Each target is tried in sequence until one succeeds.
/// If all targets fail, returns the last error.
pub struct ChainedDlqPublisher {
    targets: Vec<Arc<dyn DeadLetterPublisher>>,
}

impl ChainedDlqPublisher {
    /// Create a new chained publisher with the given targets.
    pub fn new(targets: Vec<Arc<dyn DeadLetterPublisher>>) -> Self {
        Self { targets }
    }
}

#[async_trait]
impl DeadLetterPublisher for ChainedDlqPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let mut last_error = None;

        for (i, target) in self.targets.iter().enumerate() {
            match target.publish(dead_letter.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(
                        target_index = i,
                        error = %e,
                        "DLQ target failed, trying next"
                    );
                    last_error = Some(e);
                }
            }
        }

        // All targets failed
        Err(last_error.unwrap_or(DlqError::NotConfigured))
    }

    fn is_configured(&self) -> bool {
        !self.targets.is_empty()
    }
}

#[cfg(test)]
#[path = "chained.test.rs"]
mod tests;
