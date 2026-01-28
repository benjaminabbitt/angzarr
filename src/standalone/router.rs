//! Command routing for embedded runtime.
//!
//! Dispatches commands to registered aggregate handlers via gRPC.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::Status;
use tracing::{debug, info};
use uuid::Uuid;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::orchestration::aggregate::local::LocalAggregateContext;
use crate::orchestration::aggregate::{execute_command_pipeline, parse_command_cover, PipelineMode};
use crate::proto::aggregate_client::AggregateClient;
use crate::proto::{CommandBook, CommandResponse, Cover, Uuid as ProtoUuid};
use crate::storage::{EventStore, SnapshotStore};

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
/// Routes commands to registered aggregate business logic via gRPC.
/// Each domain has its own isolated storage.
#[derive(Clone)]
pub struct CommandRouter {
    /// Business logic gRPC clients by domain.
    business_clients: Arc<HashMap<String, Arc<Mutex<AggregateClient<tonic::transport::Channel>>>>>,
    /// Per-domain storage.
    stores: Arc<HashMap<String, DomainStorage>>,
    /// Service discovery for projectors.
    discovery: Arc<dyn ServiceDiscovery>,
    /// Event bus for publishing.
    event_bus: Arc<dyn EventBus>,
}

impl CommandRouter {
    /// Create a new command router.
    pub fn new(
        business_clients: HashMap<String, Arc<Mutex<AggregateClient<tonic::transport::Channel>>>>,
        stores: HashMap<String, DomainStorage>,
        discovery: Arc<dyn ServiceDiscovery>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        let domains: Vec<_> = business_clients.keys().cloned().collect();
        info!(
            domains = ?domains,
            "Command router initialized"
        );

        Self {
            business_clients: Arc::new(business_clients),
            stores: Arc::new(stores),
            discovery,
            event_bus,
        }
    }

    /// Get list of registered domains.
    pub fn domains(&self) -> Vec<&str> {
        self.business_clients.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a domain has a registered handler.
    pub fn has_handler(&self, domain: &str) -> bool {
        self.business_clients.contains_key(domain)
    }

    /// Execute a command and return the response.
    ///
    /// Validates the command's sequence against the aggregate's current sequence
    /// (optimistic concurrency check) before running business logic.
    pub async fn execute(&self, command_book: CommandBook) -> Result<CommandResponse, Status> {
        self.execute_inner(command_book, true).await
    }

    /// Execute a command from a saga or process manager.
    ///
    /// Skips command sequence validation since sagas/PMs don't track
    /// target aggregate sequences.
    pub async fn execute_command(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.execute_inner(command_book, false)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Core command execution with optional sequence validation.
    async fn execute_inner(
        &self,
        command_book: CommandBook,
        validate_sequence: bool,
    ) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            "Executing command via gRPC"
        );

        let client = self
            .business_clients
            .get(&domain)
            .ok_or_else(|| Status::not_found(format!("No handler registered for domain: {domain}")))?;

        let storage = self
            .stores
            .get(&domain)
            .ok_or_else(|| Status::not_found(format!("No storage configured for domain: {domain}")))?;

        let ctx = LocalAggregateContext::new(
            storage.clone(),
            self.discovery.clone(),
            self.event_bus.clone(),
        );

        execute_command_pipeline(
            &ctx,
            client.as_ref(),
            command_book,
            PipelineMode::Execute { validate_sequence },
        )
        .await
    }

    /// Dry-run: execute command against temporal state without persisting.
    ///
    /// Loads aggregate state at a point in time, runs the handler, returns
    /// speculative events. No side effects — nothing persisted, nothing published.
    pub async fn dry_run(
        &self,
        command_book: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            ?as_of_sequence,
            ?as_of_timestamp,
            "Dry-run command via gRPC"
        );

        let client = self
            .business_clients
            .get(&domain)
            .ok_or_else(|| Status::not_found(format!("No handler registered for domain: {domain}")))?;

        let storage = self
            .stores
            .get(&domain)
            .ok_or_else(|| Status::not_found(format!("No storage configured for domain: {domain}")))?;

        let ctx = LocalAggregateContext::new(
            storage.clone(),
            self.discovery.clone(),
            self.event_bus.clone(),
        );

        execute_command_pipeline(
            &ctx,
            client.as_ref(),
            command_book,
            PipelineMode::DryRun {
                as_of_sequence,
                as_of_timestamp: as_of_timestamp.map(|s| s.to_string()),
            },
        )
        .await
    }

    /// Get storage for a domain.
    #[allow(clippy::result_large_err)]
    pub fn get_storage(&self, domain: &str) -> Result<&DomainStorage, Status> {
        self.stores.get(domain).ok_or_else(|| {
            Status::not_found(format!("No storage configured for domain: {domain}"))
        })
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

    #[test]
    fn test_create_command_book() {
        let root = Uuid::new_v4();
        let command = create_command_book("orders", root, "CreateOrder", vec![1, 2, 3]);

        assert_eq!(command.cover.as_ref().unwrap().domain, "orders");
        assert!(!command.pages.is_empty());
    }
}
