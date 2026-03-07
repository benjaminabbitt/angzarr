//! SagaContext for splitter pattern support.
//!
//! Provides access to destination aggregate state when one event triggers
//! commands to multiple aggregates.

use std::collections::HashMap;

use crate::proto::EventBook;
use crate::proto_ext::EventPageExt;

/// Context for saga handlers, providing access to destination aggregate state.
///
/// Used in the splitter pattern where one event triggers commands to multiple aggregates.
/// Provides sequence number lookup for optimistic concurrency control.
///
/// # Example
///
/// ```ignore
/// fn handle_table_settled(
///     evt: &TableSettled,
///     ctx: &SagaContext,
/// ) -> Vec<CommandBook> {
///     evt.payouts
///         .iter()
///         .map(|payout| {
///             let seq = ctx.get_sequence("player", &payout.player_root);
///             let cmd = TransferFunds {
///                 player_root: payout.player_root.clone(),
///                 amount: payout.amount,
///             };
///             new_command_book("player", &cmd, seq)
///         })
///         .collect()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SagaContext {
    destinations: HashMap<String, EventBook>,
}

impl SagaContext {
    /// Create a context from a slice of destination EventBooks.
    pub fn new(destination_books: &[EventBook]) -> Self {
        let mut destinations = HashMap::new();
        for book in destination_books {
            if let Some(cover) = &book.cover {
                if !cover.domain.is_empty() {
                    let root_bytes = cover
                        .root
                        .as_ref()
                        .map(|u| u.value.as_slice())
                        .unwrap_or(&[]);
                    let key = make_key(&cover.domain, root_bytes);
                    destinations.insert(key, book.clone());
                }
            }
        }
        Self { destinations }
    }

    /// Get the next sequence number for a destination aggregate.
    /// Returns 1 if the aggregate doesn't exist yet.
    pub fn get_sequence(&self, domain: &str, aggregate_root: &[u8]) -> u32 {
        let key = make_key(domain, aggregate_root);
        if let Some(book) = self.destinations.get(&key) {
            if let Some(last_page) = book.pages.last() {
                return last_page.sequence_num() + 1;
            }
        }
        1
    }

    /// Get the EventBook for a destination aggregate, if available.
    pub fn get_destination(&self, domain: &str, aggregate_root: &[u8]) -> Option<&EventBook> {
        let key = make_key(domain, aggregate_root);
        self.destinations.get(&key)
    }

    /// Check if a destination exists.
    pub fn has_destination(&self, domain: &str, aggregate_root: &[u8]) -> bool {
        let key = make_key(domain, aggregate_root);
        self.destinations.contains_key(&key)
    }
}

fn make_key(domain: &str, root: &[u8]) -> String {
    format!("{}:{}", domain, hex::encode(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{page_header, Cover, EventPage, PageHeader, Uuid};

    fn make_event_book(domain: &str, root: &[u8], events: u32) -> EventBook {
        let mut pages = Vec::new();
        for i in 1..=events {
            pages.push(EventPage {
                header: Some(PageHeader {
                    sequence_type: Some(page_header::SequenceType::Sequence(i)),
                }),
                ..Default::default()
            });
        }
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(Uuid {
                    value: root.to_vec(),
                }),
                ..Default::default()
            }),
            pages,
            ..Default::default()
        }
    }

    #[test]
    fn get_sequence_returns_1_for_empty_destinations() {
        let ctx = SagaContext::new(&[]);
        assert_eq!(ctx.get_sequence("player", b"some-root"), 1);
    }

    #[test]
    fn get_sequence_returns_next_after_last() {
        let book = make_event_book("player", b"player-1", 5);
        let ctx = SagaContext::new(&[book]);
        assert_eq!(ctx.get_sequence("player", b"player-1"), 6);
    }

    #[test]
    fn get_sequence_returns_1_for_empty_book() {
        let book = make_event_book("player", b"player-1", 0);
        let ctx = SagaContext::new(&[book]);
        assert_eq!(ctx.get_sequence("player", b"player-1"), 1);
    }

    #[test]
    fn has_destination_returns_true_when_exists() {
        let book = make_event_book("player", b"player-1", 1);
        let ctx = SagaContext::new(&[book]);
        assert!(ctx.has_destination("player", b"player-1"));
        assert!(!ctx.has_destination("player", b"player-2"));
        assert!(!ctx.has_destination("table", b"player-1"));
    }
}
