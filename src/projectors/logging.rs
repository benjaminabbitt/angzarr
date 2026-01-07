//! Logging projector for debugging and testing.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::info;

use crate::interfaces::projector::{Projector, Result};
use crate::proto::{EventBook, Projection};

/// Projector that logs all received events.
///
/// Useful for debugging event flow and testing event delivery.
pub struct LoggingProjector {
    name: String,
    domains: Vec<String>,
}

impl LoggingProjector {
    /// Create a new logging projector.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            domains: Vec::new(),
        }
    }

    /// Create a logging projector for specific domains.
    pub fn for_domains(name: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            name: name.into(),
            domains,
        }
    }
}

#[async_trait]
impl Projector for LoggingProjector {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        self.domains.clone()
    }

    async fn project(&self, book: &Arc<EventBook>) -> Result<Option<Projection>> {
        let domain = book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let root = book
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| format!("{:02x?}", &r.value[..r.value.len().min(8)]))
            .unwrap_or_else(|| "unknown".to_string());

        for (i, page) in book.pages.iter().enumerate() {
            let event_type = page
                .event
                .as_ref()
                .map(|e| e.type_url.as_str())
                .unwrap_or("unknown");

            let sequence = page
                .sequence
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "none".to_string());

            info!(
                projector = %self.name,
                domain = %domain,
                root = %root,
                sequence = %sequence,
                event_index = i,
                event_type = %event_type,
                "Event received"
            );
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, Cover, EventPage, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_event_book(domain: &str, event_count: usize) -> EventBook {
        let root = ProtoUuid {
            value: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        };

        let pages: Vec<EventPage> = (0..event_count)
            .map(|i| EventPage {
                sequence: Some(event_page::Sequence::Num(i as u32)),
                event: Some(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            })
            .collect();

        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(root),
            }),
            pages,
            snapshot: None,
        }
    }

    #[tokio::test]
    async fn test_logging_projector_processes_events() {
        let projector = LoggingProjector::new("test_logger");
        let book = Arc::new(make_event_book("orders", 3));

        let result = projector.project(&book).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_logging_projector_domain_filter() {
        let projector = LoggingProjector::for_domains("test_logger", vec!["orders".to_string()]);

        assert_eq!(projector.domains(), vec!["orders".to_string()]);
    }
}
