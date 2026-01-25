//! Command routing for embedded runtime.
//!
//! Dispatches commands to registered aggregate handlers.

use std::collections::HashMap;
use std::sync::Arc;

use prost::Message;
use tonic::Status;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::bus::EventBus;
use crate::proto::{
    event_page, CommandBook, CommandResponse, ContextualCommand, Cover, EventBook,
    Uuid as ProtoUuid,
};
use crate::storage::{EventStore, SnapshotStore, StorageError};

use super::traits::AggregateHandler;

use crate::proto::EventPage;

/// Maximum retry attempts for sequence conflicts.
const MAX_RETRY_ATTEMPTS: u32 = 5;

/// Extract sequence number from an EventPage.
fn extract_sequence(page: Option<&EventPage>) -> u32 {
    page.and_then(|p| match &p.sequence {
        Some(event_page::Sequence::Num(n)) => Some(*n),
        _ => None,
    })
    .unwrap_or(0)
}

/// Command router for embedded runtime.
///
/// Routes commands to registered aggregate handlers based on domain.
#[derive(Clone)]
pub struct CommandRouter {
    /// Registered handlers by domain.
    handlers: Arc<HashMap<String, Arc<dyn AggregateHandler>>>,
    /// Event store.
    event_store: Arc<dyn EventStore>,
    /// Snapshot store.
    snapshot_store: Arc<dyn SnapshotStore>,
    /// Event bus for publishing.
    event_bus: Arc<dyn EventBus>,
}

impl CommandRouter {
    /// Create a new command router.
    pub fn new(
        handlers: HashMap<String, Arc<dyn AggregateHandler>>,
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        let domains: Vec<_> = handlers.keys().cloned().collect();
        info!(
            domains = ?domains,
            "Command router initialized"
        );

        Self {
            handlers: Arc::new(handlers),
            event_store,
            snapshot_store,
            event_bus,
        }
    }

    /// Get list of registered domains.
    pub fn domains(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a domain has a registered handler.
    pub fn has_handler(&self, domain: &str) -> bool {
        self.handlers.contains_key(domain)
    }

    /// Execute a command and return the response.
    ///
    /// This is the main entry point for command execution.
    pub async fn execute(&self, command_book: CommandBook) -> Result<CommandResponse, Status> {
        let cover = command_book
            .cover
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

        let domain = &cover.domain;
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

        let root_uuid = Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        // Get handler for domain
        let handler = self
            .handlers
            .get(domain)
            .ok_or_else(|| Status::not_found(format!("No handler registered for domain: {}", domain)))?;

        let correlation_id = self.ensure_correlation_id(&command_book)?;
        let auto_resequence = command_book.auto_resequence;

        debug!(
            domain = %domain,
            root = %root_uuid,
            correlation_id = %correlation_id,
            "Executing command"
        );

        // Retry loop for auto_resequence
        let mut attempt = 0;
        loop {
            attempt += 1;

            // Load prior events
            let prior_events = self
                .load_prior_events(domain, root_uuid)
                .await?;

            // Create contextual command
            let contextual_command = ContextualCommand {
                events: Some(prior_events),
                command: Some(command_book.clone()),
            };

            // Call handler
            let new_events = handler.handle(contextual_command).await?;

            // Validate and persist events
            match self.persist_events(domain, root_uuid, &new_events, &correlation_id).await {
                Ok(final_events) => {
                    // Publish to event bus
                    if let Err(e) = self.event_bus.publish(Arc::new(final_events.clone())).await {
                        warn!(
                            domain = %domain,
                            root = %root_uuid,
                            error = %e,
                            "Failed to publish events"
                        );
                    }

                    return Ok(CommandResponse {
                        events: Some(final_events),
                        projections: vec![], // Sync projectors handled separately
                    });
                }
                Err(e) => {
                    if auto_resequence && attempt < MAX_RETRY_ATTEMPTS {
                        if let Some(StorageError::SequenceConflict { .. }) =
                            e.downcast_ref::<StorageError>()
                        {
                            debug!(
                                domain = %domain,
                                root = %root_uuid,
                                attempt = attempt,
                                "Sequence conflict, retrying"
                            );
                            continue;
                        }
                    }

                    error!(
                        domain = %domain,
                        root = %root_uuid,
                        error = %e,
                        "Command execution failed"
                    );

                    return Err(Status::internal(format!("Command execution failed: {e}")));
                }
            }
        }
    }

    /// Execute a command from a saga.
    ///
    /// This is called when a saga emits commands to other aggregates.
    pub async fn execute_command(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.execute(command_book)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Load prior events for an aggregate.
    async fn load_prior_events(
        &self,
        domain: &str,
        root: Uuid,
    ) -> Result<EventBook, Status> {
        // Try to load snapshot first
        let snapshot = self
            .snapshot_store
            .get(domain, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to load snapshot: {e}")))?;

        let (events, snapshot_data) = if let Some(snap) = snapshot {
            let from_seq = snap.sequence + 1;
            let events = self
                .event_store
                .get_from(domain, root, from_seq)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
            (events, Some(snap))
        } else {
            let events = self
                .event_store
                .get(domain, root)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
            (events, None)
        };

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: events,
            snapshot: snapshot_data,
            correlation_id: String::new(),
            snapshot_state: None,
        })
    }

    /// Persist new events to storage.
    async fn persist_events(
        &self,
        domain: &str,
        root: Uuid,
        events: &EventBook,
        correlation_id: &str,
    ) -> Result<EventBook, Box<dyn std::error::Error + Send + Sync>> {
        if events.pages.is_empty() {
            // No events to persist (command was a no-op)
            return Ok(EventBook {
                cover: events.cover.clone(),
                pages: vec![],
                snapshot: None,
                correlation_id: correlation_id.to_string(),
                snapshot_state: None,
            });
        }

        // Validate sequence
        let next_sequence = self.event_store.get_next_sequence(domain, root).await?;
        let first_event_seq = extract_sequence(events.pages.first());

        if first_event_seq != next_sequence {
            return Err(Box::new(StorageError::SequenceConflict {
                expected: next_sequence,
                actual: first_event_seq,
            }));
        }

        // Persist events
        self.event_store
            .add(domain, root, events.pages.clone())
            .await?;

        // Persist snapshot if present
        if let Some(ref snapshot_state) = events.snapshot_state {
            let last_seq = extract_sequence(events.pages.last());
            let snapshot = crate::proto::Snapshot {
                sequence: last_seq,
                state: Some(snapshot_state.clone()),
            };
            self.snapshot_store.put(domain, root, snapshot).await?;
        }

        // Return events with correlation ID
        Ok(EventBook {
            cover: events.cover.clone(),
            pages: events.pages.clone(),
            snapshot: None,
            correlation_id: correlation_id.to_string(),
            snapshot_state: events.snapshot_state.clone(),
        })
    }

    /// Ensure correlation ID exists, generating one if needed.
    #[allow(clippy::result_large_err)]
    fn ensure_correlation_id(&self, command_book: &CommandBook) -> Result<String, Status> {
        if !command_book.correlation_id.is_empty() {
            return Ok(command_book.correlation_id.clone());
        }

        // Generate deterministic correlation ID from command content
        let mut buf = Vec::new();
        command_book
            .encode(&mut buf)
            .map_err(|e| Status::internal(format!("Failed to encode command: {e}")))?;

        let angzarr_ns = Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"angzarr.dev");
        let correlation_id = Uuid::new_v5(&angzarr_ns, &buf).to_string();

        Ok(correlation_id)
    }
}

/// Helper to create a command book.
#[allow(dead_code)]
pub fn create_command_book(
    domain: &str,
    root: Uuid,
    command_type: &str,
    command_data: Vec<u8>,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: vec![crate::proto::CommandPage {
            sequence: 0, // Will be set by router
            command: Some(prost_types::Any {
                type_url: command_type.to_string(),
                value: command_data,
            }),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: true,
        fact: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_cover(domain: &str, root: Uuid) -> Cover {
        Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }
    }

    #[test]
    fn test_create_command_book() {
        let root = Uuid::new_v4();
        let command = create_command_book("orders", root, "CreateOrder", vec![1, 2, 3]);

        assert_eq!(command.cover.as_ref().unwrap().domain, "orders");
        assert!(!command.pages.is_empty());
        assert!(command.auto_resequence);
    }
}
