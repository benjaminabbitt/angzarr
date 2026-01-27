//! Command routing for embedded runtime.
//!
//! Dispatches commands to registered aggregate handlers.

use std::collections::HashMap;
use std::sync::Arc;

use prost::Message;
use tonic::Status;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use tokio::sync::RwLock;

use crate::bus::EventBus;
use crate::proto::{
    event_page, CommandBook, CommandResponse, ContextualCommand, Cover, EventBook, Projection,
    Uuid as ProtoUuid,
};
use crate::storage::{EventStore, SnapshotStore, StorageError};

use super::traits::{AggregateHandler, ProjectorConfig, ProjectorHandler};

use crate::proto::EventPage;

/// Entry for a registered sync projector.
struct SyncProjectorEntry {
    name: String,
    handler: Arc<dyn ProjectorHandler>,
    config: ProjectorConfig,
}

/// Extract sequence number from an EventPage.
fn extract_sequence(page: Option<&EventPage>) -> u32 {
    page.and_then(|p| match &p.sequence {
        Some(event_page::Sequence::Num(n)) => Some(*n),
        _ => None,
    })
    .unwrap_or(0)
}

/// Per-domain storage.
#[derive(Clone)]
pub struct DomainStorage {
    /// Event store for this domain.
    pub event_store: Arc<dyn EventStore>,
    /// Snapshot store for this domain.
    pub snapshot_store: Arc<dyn SnapshotStore>,
}

/// Command router for embedded runtime.
///
/// Routes commands to registered aggregate handlers based on domain.
/// Each domain has its own isolated storage.
#[derive(Clone)]
pub struct CommandRouter {
    /// Registered handlers by domain.
    handlers: Arc<HashMap<String, Arc<dyn AggregateHandler>>>,
    /// Per-domain storage.
    stores: Arc<HashMap<String, DomainStorage>>,
    /// Event bus for publishing.
    event_bus: Arc<dyn EventBus>,
    /// Synchronous projectors (called during command execution).
    sync_projectors: Arc<RwLock<Vec<SyncProjectorEntry>>>,
}

impl CommandRouter {
    /// Create a new command router with per-domain storage.
    pub fn new(
        handlers: HashMap<String, Arc<dyn AggregateHandler>>,
        stores: HashMap<String, DomainStorage>,
        event_bus: Arc<dyn EventBus>,
        sync_projectors: Vec<(String, Arc<dyn ProjectorHandler>, ProjectorConfig)>,
    ) -> Self {
        let domains: Vec<_> = handlers.keys().cloned().collect();
        info!(
            domains = ?domains,
            sync_projectors = sync_projectors.len(),
            "Command router initialized"
        );

        let sync_entries: Vec<SyncProjectorEntry> = sync_projectors
            .into_iter()
            .filter(|(_, _, config)| config.synchronous)
            .map(|(name, handler, config)| SyncProjectorEntry {
                name,
                handler,
                config,
            })
            .collect();

        Self {
            handlers: Arc::new(handlers),
            stores: Arc::new(stores),
            event_bus,
            sync_projectors: Arc::new(RwLock::new(sync_entries)),
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
        let handler = self.handlers.get(domain).ok_or_else(|| {
            Status::not_found(format!("No handler registered for domain: {}", domain))
        })?;

        let correlation_id = self.ensure_correlation_id(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            correlation_id = %correlation_id,
            "Executing command"
        );

        // Load prior events
        let prior_events = self.load_prior_events(domain, root_uuid).await?;

        // Create contextual command
        let contextual_command = ContextualCommand {
            events: Some(prior_events),
            command: Some(command_book.clone()),
        };

        // Call handler
        let new_events = handler.handle(contextual_command).await?;

        // Validate and persist events
        match self
            .persist_events(domain, root_uuid, &new_events, &correlation_id)
            .await
        {
            Ok(final_events) => {
                // Call sync projectors before publishing
                let projections = self.call_sync_projectors(&final_events).await;

                // Publish to event bus for async consumers
                if let Err(e) = self.event_bus.publish(Arc::new(final_events.clone())).await {
                    warn!(
                        domain = %domain,
                        root = %root_uuid,
                        error = %e,
                        "Failed to publish events"
                    );
                }

                Ok(CommandResponse {
                    events: Some(final_events),
                    projections,
                })
            }
            Err(e) => {
                error!(
                    domain = %domain,
                    root = %root_uuid,
                    error = %e,
                    "Command execution failed"
                );

                Err(Status::internal(format!("Command execution failed: {e}")))
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

    /// Get storage for a domain.
    #[allow(clippy::result_large_err)]
    fn get_storage(&self, domain: &str) -> Result<&DomainStorage, Status> {
        self.stores.get(domain).ok_or_else(|| {
            Status::not_found(format!("No storage configured for domain: {}", domain))
        })
    }

    /// Load prior events for an aggregate.
    async fn load_prior_events(&self, domain: &str, root: Uuid) -> Result<EventBook, Status> {
        let storage = self.get_storage(domain)?;

        // Try to load snapshot first
        let snapshot = storage
            .snapshot_store
            .get(domain, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to load snapshot: {e}")))?;

        let (events, snapshot_data) = if let Some(snap) = snapshot {
            let from_seq = snap.sequence + 1;
            let events = storage
                .event_store
                .get_from(domain, root, from_seq)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
            (events, Some(snap))
        } else {
            let events = storage
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
                correlation_id: String::new(),
            }),
            pages: events,
            snapshot: snapshot_data,
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
            // Ensure correlation_id is set on cover
            let cover = events.cover.clone().map(|mut c| {
                if c.correlation_id.is_empty() {
                    c.correlation_id = correlation_id.to_string();
                }
                c
            });
            return Ok(EventBook {
                cover,
                pages: vec![],
                snapshot: None,
                snapshot_state: None,
            });
        }

        let storage = self
            .stores
            .get(domain)
            .ok_or_else(|| format!("No storage configured for domain: {}", domain))?;

        // Validate sequence
        let next_sequence = storage.event_store.get_next_sequence(domain, root).await?;
        let first_event_seq = extract_sequence(events.pages.first());

        if first_event_seq != next_sequence {
            return Err(Box::new(StorageError::SequenceConflict {
                expected: next_sequence,
                actual: first_event_seq,
            }));
        }

        // Persist events
        storage
            .event_store
            .add(domain, root, events.pages.clone(), correlation_id)
            .await?;

        // Persist snapshot if present
        if let Some(ref snapshot_state) = events.snapshot_state {
            let last_seq = extract_sequence(events.pages.last());
            let snapshot = crate::proto::Snapshot {
                sequence: last_seq,
                state: Some(snapshot_state.clone()),
            };
            storage.snapshot_store.put(domain, root, snapshot).await?;
        }

        // Return events with correlation ID set on cover
        let cover = events.cover.clone().map(|mut c| {
            if c.correlation_id.is_empty() {
                c.correlation_id = correlation_id.to_string();
            }
            c
        });
        Ok(EventBook {
            cover,
            pages: events.pages.clone(),
            snapshot: None,
            snapshot_state: events.snapshot_state.clone(),
        })
    }

    /// Ensure correlation ID exists, generating one if needed.
    #[allow(clippy::result_large_err)]
    fn ensure_correlation_id(&self, command_book: &CommandBook) -> Result<String, Status> {
        let existing = command_book
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");

        if !existing.is_empty() {
            return Ok(existing.to_string());
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

    /// Call synchronous projectors and collect their results.
    async fn call_sync_projectors(&self, events: &EventBook) -> Vec<Projection> {
        let projectors = self.sync_projectors.read().await;
        let mut projections = Vec::new();

        let domain = events
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        for entry in projectors.iter() {
            // Check domain filter
            if !entry.config.domains.is_empty() && !entry.config.domains.iter().any(|d| d == domain)
            {
                continue;
            }

            match entry.handler.handle(events).await {
                Ok(projection) => {
                    projections.push(projection);
                }
                Err(e) => {
                    error!(
                        projector = %entry.name,
                        domain = %domain,
                        error = %e,
                        "Synchronous projector failed"
                    );
                }
            }
        }

        projections
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
            correlation_id: String::new(),
        }),
        pages: vec![crate::proto::CommandPage {
            sequence: 0,
            command: Some(prost_types::Any {
                type_url: command_type.to_string(),
                value: command_data,
            }),
        }],
        saga_origin: None,
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
            correlation_id: String::new(),
        }
    }

    #[test]
    fn test_create_command_book() {
        let root = Uuid::new_v4();
        let command = create_command_book("orders", root, "CreateOrder", vec![1, 2, 3]);

        assert_eq!(command.cover.as_ref().unwrap().domain, "orders");
        assert!(!command.pages.is_empty());
    }
}
