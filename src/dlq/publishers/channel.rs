//! In-memory channel DLQ publisher.
//!
//! Used for in-process mode and testing.
//!
//! # Manual Setup Required
//!
//! This publisher is **not available via the DLQ factory** (`init_dlq_publisher`).
//! The factory pattern returns only the publisher, but channel-based DLQ requires
//! both a publisher AND a receiver for consuming dead letters.
//!
//! Create manually with [`ChannelDeadLetterPublisher::new()`]:
//!
//! ```rust,ignore
//! let (publisher, receiver) = ChannelDeadLetterPublisher::new();
//!
//! // Use publisher for sending dead letters
//! let dlq: Arc<dyn DeadLetterPublisher> = Arc::new(publisher);
//!
//! // Use receiver to consume dead letters
//! tokio::spawn(async move {
//!     while let Some(dead_letter) = receiver.recv().await {
//!         // Process dead letter
//!     }
//! });
//! ```
//!
//! For configuration-driven DLQ setup, use other backends (filesystem, database,
//! logging) which don't require a receiver.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

use super::super::{AngzarrDeadLetter, DeadLetterPublisher, DlqError};

/// In-memory DLQ publisher using a channel.
///
/// Used for in-process mode and testing. Requires manual instantiation via
/// [`new()`](Self::new) because the caller needs the receiver to consume
/// dead letters. Not available via the DLQ factory.
pub struct ChannelDeadLetterPublisher {
    sender: mpsc::UnboundedSender<AngzarrDeadLetter>,
}

impl ChannelDeadLetterPublisher {
    /// Create a new channel-based DLQ publisher.
    ///
    /// Returns the publisher and a receiver for consuming dead letters.
    /// This is the only way to create a channel publisher—it cannot be
    /// created via the DLQ factory because the factory has no way to
    /// return the receiver.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<AngzarrDeadLetter>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }
}

#[async_trait]
impl DeadLetterPublisher for ChannelDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        #[cfg(feature = "otel")]
        let start = std::time::Instant::now();

        info!(
            topic = %dead_letter.topic(),
            reason = %dead_letter.rejection_reason,
            "Publishing to channel DLQ"
        );

        #[cfg(feature = "otel")]
        let domain = dead_letter.domain().unwrap_or("unknown").to_string();
        #[cfg(feature = "otel")]
        let reason_type = dead_letter.reason_type();

        let result = self
            .sender
            .send(dead_letter)
            .map_err(|e| DlqError::PublishFailed(e.to_string()));

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                self, backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_DURATION,
                DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[metrics::backend_attr("channel")],
            );
            if result.is_ok() {
                DLQ_PUBLISH_TOTAL.add(
                    1,
                    &[
                        domain_attr(&domain),
                        reason_type_attr(reason_type),
                        backend_attr("channel"),
                    ],
                );
            }
        }

        result
    }
}

#[cfg(test)]
#[path = "channel.test.rs"]
mod tests;
