//! Customer Log Projector - Rust Implementation.
//!
//! Pretty prints customer events to terminal.

use std::sync::Arc;

use angzarr::async_trait::async_trait;
use angzarr::interfaces::projector::{Projector, Result};
use angzarr::proto::EventBook;

/// Result of processing an event through the log projector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogResult {
    /// Event was logged successfully
    Logged,
    /// Event type was unknown
    Unknown,
}

/// Projector that logs customer events.
#[derive(Debug)]
pub struct CustomerLogProjector {
    name: String,
}

impl CustomerLogProjector {
    /// Create a new customer log projector.
    pub fn new() -> Self {
        Self {
            name: "log-customer".to_string(),
        }
    }

    /// Process an event and return the result for testing.
    /// Returns LogResult::Logged for known event types, LogResult::Unknown for unknown types.
    pub fn process_event(&self, type_url: &str, data: &[u8]) -> LogResult {
        let event_type = type_url.rsplit('.').next().unwrap_or(type_url);

        match event_type {
            "CustomerCreated" | "LoyaltyPointsAdded" | "LoyaltyPointsRedeemed" => {
                common::log_event("customer", "test", 0, type_url, data);
                LogResult::Logged
            }
            _ => {
                println!("[{}] Unknown event type: {}", self.name, event_type);
                LogResult::Unknown
            }
        }
    }
}

impl Default for CustomerLogProjector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projector for CustomerLogProjector {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        vec!["customer".to_string()]
    }

    async fn project(&self, book: &Arc<EventBook>) -> Result<Option<angzarr::proto::Projection>> {
        let domain = book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let root_id = book
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| hex::encode(&r.value))
            .unwrap_or_default();

        let short_root_id = &root_id[..16.min(root_id.len())];

        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            let sequence = page.sequence.as_ref().map_or(0, |s| match s {
                angzarr::proto::event_page::Sequence::Num(n) => *n,
                angzarr::proto::event_page::Sequence::Force(_) => 0,
            });

            common::log_event(
                domain,
                short_root_id,
                sequence,
                &event.type_url,
                &event.value,
            );
        }

        // Log projector doesn't produce a projection
        Ok(None)
    }

    fn is_synchronous(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projector_name() {
        let projector = CustomerLogProjector::new();
        assert_eq!(projector.name(), "log-customer");
    }

    #[test]
    fn test_projector_domains() {
        let projector = CustomerLogProjector::new();
        assert_eq!(projector.domains(), vec!["customer"]);
    }
}
