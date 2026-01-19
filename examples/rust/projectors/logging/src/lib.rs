//! Logging projector for debugging and demonstration.
//!
//! Receives events and logs them with structured tracing output.
//! Useful for observing event flow without database dependencies.

use angzarr::proto::{EventBook, Projection};
use common::ProjectorLogic;
use tonic::Status;
use tracing::info;

pub const PROJECTOR_NAME: &str = "logging";

/// A projector that logs all received events.
pub struct LoggingProjector {
    domain: String,
}

impl LoggingProjector {
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
        }
    }

    fn extract_event_type(event: &prost_types::Any) -> String {
        event
            .type_url
            .rsplit('/')
            .next()
            .unwrap_or(&event.type_url)
            .to_string()
    }
}

#[tonic::async_trait]
impl ProjectorLogic for LoggingProjector {
    async fn handle(&self, book: &EventBook) -> Result<Option<Projection>, Status> {
        let cover = book.cover.as_ref();
        let domain = cover.map(|c| c.domain.as_str()).unwrap_or("unknown");
        let root_id = cover
            .and_then(|c| c.root.as_ref())
            .map(|r| hex::encode(&r.value))
            .unwrap_or_else(|| "none".to_string());

        for page in &book.pages {
            if let Some(event) = &page.event {
                let event_type = Self::extract_event_type(event);
                let sequence = page.sequence.as_ref().and_then(|s| match s {
                    angzarr::proto::event_page::Sequence::Num(n) => Some(*n),
                    angzarr::proto::event_page::Sequence::Force(_) => None,
                });

                info!(
                    projector = PROJECTOR_NAME,
                    target_domain = %self.domain,
                    event_domain = %domain,
                    root_id = %root_id,
                    event_type = %event_type,
                    sequence = ?sequence,
                    payload_size = event.value.len(),
                    "event_received"
                );
            }
        }

        // Logging projector doesn't produce projection output
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logging_projector_handles_empty_book() {
        let projector = LoggingProjector::new("test");
        let book = EventBook::default();
        let result = projector.handle(&book).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
