//! Logging-based DLQ publisher.
//!
//! Logs dead letters at WARN level with structured fields.
//! Useful for observability and as a last-resort fallback target.

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
                if dlq_type != "logging" {
                    return None;
                }
                Some(Ok(Arc::new(LoggingDeadLetterPublisher) as Arc<dyn DeadLetterPublisher>))
            })
        },
    }
}

/// Logging-based DLQ publisher.
///
/// Logs dead letters at WARN level with structured fields for observability.
/// Unlike NoopDeadLetterPublisher, this reports itself as configured.
pub struct LoggingDeadLetterPublisher;

#[async_trait]
impl DeadLetterPublisher for LoggingDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let domain = dead_letter.domain().unwrap_or("unknown");
        let correlation_id = dead_letter
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");

        warn!(
            domain = %domain,
            correlation_id = %correlation_id,
            reason = %dead_letter.rejection_reason,
            reason_type = %dead_letter.reason_type(),
            source_component = %dead_letter.source_component,
            source_component_type = %dead_letter.source_component_type,
            "Dead letter published to logging DLQ"
        );

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(domain),
                    reason_type_attr(dead_letter.reason_type()),
                    backend_attr("logging"),
                ],
            );
        }

        Ok(())
    }

    fn is_configured(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dlq::{AngzarrDeadLetter, DeadLetterPayload};
    use crate::proto::{
        command_page, CommandBook, CommandPage, Cover, MergeStrategy, Uuid as ProtoUuid,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_test_command(domain: &str) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-corr-123".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                payload: Some(command_page::Payload::Command(prost_types::Any {
                    type_url: "test.Command".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeManual as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_dead_letter(domain: &str, reason: &str) -> AngzarrDeadLetter {
        let cmd = make_test_command(domain);
        AngzarrDeadLetter {
            cover: cmd.cover.clone(),
            payload: DeadLetterPayload::Command(cmd),
            rejection_reason: reason.to_string(),
            rejection_details: None,
            occurred_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            metadata: HashMap::new(),
            source_component: "test-component".to_string(),
            source_component_type: "aggregate".to_string(),
        }
    }

    // NOTE: is_configured() and basic publish() tests are covered by
    // tests/interfaces/features/dlq_publishers.feature (Gherkin contract tests).
    // Only implementation-specific edge cases remain here.

    #[tokio::test]
    async fn test_logging_publisher_handles_missing_correlation() {
        let publisher = LoggingDeadLetterPublisher;
        let mut dead_letter = make_dead_letter("orders", "Test rejection");
        dead_letter.cover = None; // No cover means no correlation ID

        let result = publisher.publish(dead_letter).await;

        assert!(result.is_ok(), "Should handle missing cover gracefully");
    }
}
