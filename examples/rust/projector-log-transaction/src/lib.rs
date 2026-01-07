//! Transaction Log Projector - Rust Implementation.
//!
//! Pretty prints transaction events to terminal.

use std::sync::Arc;

use evented::async_trait::async_trait;
use evented::interfaces::projector::{Projector, Result};
use evented::proto::EventBook;

/// Projector that logs transaction events.
pub struct TransactionLogProjector {
    name: String,
}

impl TransactionLogProjector {
    /// Create a new transaction log projector.
    pub fn new() -> Self {
        Self {
            name: "log-transaction".to_string(),
        }
    }
}

impl Default for TransactionLogProjector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projector for TransactionLogProjector {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        vec!["transaction".to_string()]
    }

    async fn project(
        &self,
        book: &Arc<EventBook>,
    ) -> Result<Option<evented::proto::Projection>> {
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
                evented::proto::event_page::Sequence::Num(n) => *n,
                evented::proto::event_page::Sequence::Force(_) => 0,
            });

            common::log_event(domain, short_root_id, sequence, &event.type_url, &event.value);
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
        let projector = TransactionLogProjector::new();
        assert_eq!(projector.name(), "log-transaction");
    }

    #[test]
    fn test_projector_domains() {
        let projector = TransactionLogProjector::new();
        assert_eq!(projector.domains(), vec!["transaction"]);
    }
}
