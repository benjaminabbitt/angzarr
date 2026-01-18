//! Shared EventBook repair logic for coordinators.
//!
//! Provides a reusable component for projector and saga coordinators
//! to detect and repair incomplete EventBooks.

use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::Status;
use tracing::{error, info, warn};

use crate::proto::EventBook;
use crate::services::event_book_repair::{self, EventBookRepairer};

/// Component for repairing incomplete EventBooks.
///
/// Wraps an optional EventBookRepairer and provides consistent
/// repair logic for coordinator services.
pub struct RepairableCoordinator {
    repairer: Option<Arc<Mutex<EventBookRepairer>>>,
}

impl RepairableCoordinator {
    /// Create without repair capability.
    pub fn new() -> Self {
        Self { repairer: None }
    }

    /// Create with repair capability.
    ///
    /// Connects to the EventQuery service at the given address.
    pub async fn with_repair(event_query_address: &str) -> Result<Self, String> {
        let repairer = EventBookRepairer::connect(event_query_address)
            .await
            .map_err(|e| format!("Failed to connect to EventQuery service: {}", e))?;

        info!(
            address = %event_query_address,
            "Connected to EventQuery service for EventBook repair"
        );

        Ok(Self {
            repairer: Some(Arc::new(Mutex::new(repairer))),
        })
    }

    /// Create with an existing repairer.
    pub fn with_repairer(repairer: EventBookRepairer) -> Self {
        Self {
            repairer: Some(Arc::new(Mutex::new(repairer))),
        }
    }

    /// Repair an EventBook if incomplete.
    ///
    /// If a repairer is configured and the EventBook is incomplete,
    /// fetches the complete history from the EventQuery service.
    pub async fn repair_event_book(&self, event_book: EventBook) -> Result<EventBook, Status> {
        if event_book_repair::is_complete(&event_book) {
            return Ok(event_book);
        }

        let Some(ref repairer) = self.repairer else {
            warn!(
                "Received incomplete EventBook but no repairer configured, passing through as-is"
            );
            return Ok(event_book);
        };

        let mut repairer = repairer.lock().await;
        repairer.repair(event_book).await.map_err(|e| {
            error!(error = %e, "Failed to repair EventBook");
            Status::internal(format!("Failed to repair EventBook: {}", e))
        })
    }
}

impl Default for RepairableCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, Cover, EventPage, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_complete_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    fn make_incomplete_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(5)),
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_complete_book_passes_through() {
        let coord = RepairableCoordinator::new();
        let book = make_complete_event_book();

        let result = coord.repair_event_book(book.clone()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().pages.len(), book.pages.len());
    }

    #[tokio::test]
    async fn test_incomplete_without_repairer_passes_through() {
        let coord = RepairableCoordinator::new();
        let book = make_incomplete_event_book();

        let result = coord.repair_event_book(book.clone()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().pages.len(), 1);
    }
}
