//! No-op DLQ publisher.
//!
//! Logs dead letters but doesn't actually send them anywhere.
//! Used when DLQ is not configured or for testing.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use super::super::error::DlqError;
use super::super::factory::DlqBackend;
use super::super::{AngzarrDeadLetter, DeadLetterPublisher};

// ============================================================================
// Self-Registration
// ============================================================================

inventory::submit! {
    DlqBackend {
        try_create: |config| {
            let dlq_type = config.dlq_type.clone();
            Box::pin(async move {
                if dlq_type != "noop" {
                    return None;
                }
                Some(Ok(Arc::new(NoopDeadLetterPublisher) as Arc<dyn DeadLetterPublisher>))
            })
        },
    }
}

/// No-op DLQ publisher that logs but doesn't actually send anywhere.
///
/// Used when DLQ is not configured or for testing.
pub struct NoopDeadLetterPublisher;

#[async_trait]
impl DeadLetterPublisher for NoopDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        warn!(
            topic = %dead_letter.topic(),
            reason = %dead_letter.rejection_reason,
            source = %dead_letter.source_component,
            "DLQ not configured, logging dead letter"
        );
        Ok(())
    }

    fn is_configured(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "noop.test.rs"]
mod tests;
