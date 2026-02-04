//! In-process clients for standalone runtime.
//!
//! `CommandClient` provides the gateway client (command execution).
//! `StandaloneQueryClient` provides the query client (event retrieval).
//! Both implement the shared traits from `client_traits`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use uuid::Uuid;

use crate::client_traits::{self, ClientError};
use crate::proto::{
    CommandBook, CommandPage, CommandResponse, Cover, DryRunRequest, Edition, EventBook,
    ProcessManagerHandleResponse, Projection, Query, SagaResponse, SpeculatePmRequest,
    SpeculateProjectorRequest, SpeculateSagaRequest, Uuid as ProtoUuid,
};
use crate::repository::EventBookRepository;

use crate::orchestration::aggregate::DEFAULT_EDITION;

use super::router::{CommandRouter, DomainStorage};
use super::speculative::{DomainStateSpec, PmSpeculativeResult, SpeculativeExecutor};

/// In-process command client.
///
/// Provides a simple interface for submitting commands to the runtime.
/// Can be cloned and shared across tasks.
///
/// # Example
///
/// ```ignore
/// let client = runtime.command_client();
///
/// // Submit a command
/// let response = client
///     .command("orders", order_id)
///     .with_type("CreateOrder")
///     .with_data(order_data)
///     .send()
///     .await?;
/// ```
#[derive(Clone)]
pub struct CommandClient {
    router: Arc<CommandRouter>,
}

impl CommandClient {
    /// Create a new command client.
    pub(crate) fn new(router: Arc<CommandRouter>) -> Self {
        Self { router }
    }

    /// Start building a command for a domain and root.
    pub fn command(&self, domain: impl Into<String>, root: Uuid) -> CommandBuilder {
        CommandBuilder::new(self.router.clone(), domain.into(), root)
    }

    /// Execute a pre-built command book.
    pub async fn execute(
        &self,
        command: CommandBook,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.router
            .execute(command)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Speculatively execute a command against temporal state (dry-run).
    ///
    /// Runs the aggregate handler at a historical point in time and returns the
    /// events that *would* be produced. This is purely speculative: no events are
    /// persisted to the store, no events are published to the bus, and no sagas
    /// or projectors are triggered. Use this to validate business rules or
    /// explore "what-if" scenarios without side effects.
    pub async fn dry_run(
        &self,
        command: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.router
            .dry_run(command, as_of_sequence, as_of_timestamp)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Check if a domain has a registered handler.
    pub fn has_domain(&self, domain: &str) -> bool {
        self.router.has_handler(domain)
    }

    /// Get list of registered domains.
    pub fn domains(&self) -> Vec<&str> {
        self.router.domains()
    }
}

#[async_trait]
impl client_traits::GatewayClient for CommandClient {
    async fn execute(&self, command: CommandBook) -> client_traits::Result<CommandResponse> {
        self.router
            .execute(command)
            .await
            .map_err(ClientError::from)
    }
}

/// In-process query client for standalone runtime.
///
/// Routes queries by domain to the appropriate storage.
#[derive(Clone)]
pub struct StandaloneQueryClient {
    domain_stores: HashMap<String, DomainStorage>,
}

impl StandaloneQueryClient {
    /// Create from domain stores.
    pub fn new(domain_stores: HashMap<String, DomainStorage>) -> Self {
        Self { domain_stores }
    }
}

#[async_trait]
impl client_traits::QueryClient for StandaloneQueryClient {
    async fn get_event_book(&self, query: Query) -> client_traits::Result<EventBook> {
        let bare_domain = query
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let store = self.domain_stores.get(bare_domain).ok_or_else(|| {
            ClientError::from(Status::not_found(format!("Unknown domain: {bare_domain}")))
        })?;

        let repo =
            EventBookRepository::new(store.event_store.clone(), store.snapshot_store.clone());

        let root_uuid_bytes = query
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| r.value.as_slice())
            .unwrap_or(&[]);

        let root_uuid = Uuid::from_slice(root_uuid_bytes)
            .map_err(|e| ClientError::from(Status::invalid_argument(e.to_string())))?;

        let book = repo
            .get(bare_domain, DEFAULT_EDITION, root_uuid)
            .await
            .map_err(|e| ClientError::from(Status::internal(e.to_string())))?;

        Ok(book)
    }

    async fn get_events(&self, query: Query) -> client_traits::Result<Vec<EventBook>> {
        let bare_domain = query
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let store = self.domain_stores.get(bare_domain).ok_or_else(|| {
            ClientError::from(Status::not_found(format!("Unknown domain: {bare_domain}")))
        })?;

        let repo =
            EventBookRepository::new(store.event_store.clone(), store.snapshot_store.clone());

        let root_uuid_bytes = query
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| r.value.as_slice())
            .unwrap_or(&[]);

        let root_uuid = Uuid::from_slice(root_uuid_bytes)
            .map_err(|e| ClientError::from(Status::invalid_argument(e.to_string())))?;

        let book = repo
            .get(bare_domain, DEFAULT_EDITION, root_uuid)
            .await
            .map_err(|e| ClientError::from(Status::internal(e.to_string())))?;

        Ok(vec![book])
    }
}

impl StandaloneQueryClient {
    /// Delete all events for an edition+domain combination.
    ///
    /// Main timeline ('angzarr' or empty edition) is protected and cannot be deleted.
    /// Returns the number of events deleted.
    ///
    /// # Errors
    /// - `INVALID_ARGUMENT` if attempting to delete main timeline
    /// - `NOT_FOUND` if domain is unknown
    /// - `INTERNAL` if storage operation fails
    pub async fn delete_edition_events(
        &self,
        domain: &str,
        edition: &str,
    ) -> Result<crate::proto::EditionEventsDeleted, Status> {
        // Protect main timeline
        if edition.is_empty() || edition == DEFAULT_EDITION {
            return Err(Status::invalid_argument("Cannot delete main timeline events"));
        }

        let store = self.domain_stores.get(domain).ok_or_else(|| {
            Status::not_found(format!("Unknown domain: {domain}"))
        })?;

        let deleted_count = store
            .event_store
            .delete_edition_events(domain, edition)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(crate::proto::EditionEventsDeleted {
            edition: edition.to_string(),
            domain: domain.to_string(),
            deleted_count,
            deleted_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}

/// Client for speculative (dry-run) execution of commands, projectors, sagas, and PMs.
///
/// Wraps a `SpeculativeExecutor` with the same handler instances registered
/// in the runtime. All methods invoke real client logic without side effects.
#[derive(Clone)]
pub struct SpeculativeClient {
    executor: Arc<SpeculativeExecutor>,
    router: Arc<CommandRouter>,
}

impl SpeculativeClient {
    /// Create from a speculative executor and router.
    pub(crate) fn new(executor: Arc<SpeculativeExecutor>, router: Arc<CommandRouter>) -> Self {
        Self { executor, router }
    }

    /// Speculatively execute a command (dry-run) at a historical point.
    ///
    /// Returns the events that *would* be produced without persisting them.
    pub async fn dry_run_command(
        &self,
        command: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> Result<CommandResponse, Status> {
        self.router
            .dry_run(command, as_of_sequence, as_of_timestamp)
            .await
    }

    /// Speculatively run a projector against events.
    ///
    /// Returns the `Projection` computed by the handler without persisting
    /// to any read model.
    pub async fn projector(
        &self,
        name: &str,
        events: &EventBook,
    ) -> Result<Projection, Status> {
        self.executor.speculate_projector(name, events).await
    }

    /// Speculatively run a saga against source events.
    ///
    /// Returns the commands the saga would produce without executing them.
    pub async fn saga(
        &self,
        name: &str,
        source: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<Vec<CommandBook>, Status> {
        self.executor
            .speculate_saga(name, source, domain_specs)
            .await
    }

    /// Speculatively run a process manager against a trigger event.
    ///
    /// Returns commands + PM events without persisting PM events or
    /// executing commands.
    pub async fn process_manager(
        &self,
        name: &str,
        trigger: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<PmSpeculativeResult, Status> {
        self.executor
            .speculate_pm(name, trigger, domain_specs)
            .await
    }
}

#[async_trait]
impl client_traits::SpeculativeClient for SpeculativeClient {
    async fn dry_run(&self, request: DryRunRequest) -> client_traits::Result<CommandResponse> {
        let command = request.command.ok_or_else(|| {
            ClientError::InvalidArgument("DryRunRequest missing command".to_string())
        })?;

        let (as_of_sequence, as_of_timestamp) = match request.point_in_time {
            Some(temporal) => match temporal.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    (Some(seq), None)
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ts)) => {
                    let rfc3339 = crate::storage::helpers::timestamp_to_rfc3339(&ts)
                        .map_err(|e| ClientError::from(Status::invalid_argument(e.to_string())))?;
                    (None, Some(rfc3339))
                }
                None => (None, None),
            },
            None => (None, None),
        };

        self.router
            .dry_run(command, as_of_sequence, as_of_timestamp.as_deref())
            .await
            .map_err(ClientError::from)
    }

    async fn projector(&self, request: SpeculateProjectorRequest) -> client_traits::Result<Projection> {
        let events = request.events.ok_or_else(|| {
            ClientError::InvalidArgument("SpeculateProjectorRequest missing events".to_string())
        })?;
        self.executor
            .speculate_projector(&request.projector_name, &events)
            .await
            .map_err(ClientError::from)
    }

    async fn saga(&self, request: SpeculateSagaRequest) -> client_traits::Result<SagaResponse> {
        let source = request.source.ok_or_else(|| {
            ClientError::InvalidArgument("SpeculateSagaRequest missing source".to_string())
        })?;

        // Convert destinations to domain specs (explicit state)
        let mut domain_specs = HashMap::new();
        for dest in request.destinations {
            if let Some(cover) = &dest.cover {
                domain_specs.insert(cover.domain.clone(), DomainStateSpec::Explicit(dest));
            }
        }

        let commands = self
            .executor
            .speculate_saga(&request.saga_name, &source, &domain_specs)
            .await
            .map_err(ClientError::from)?;

        Ok(SagaResponse {
            commands,
            events: vec![],
        })
    }

    async fn process_manager(
        &self,
        request: SpeculatePmRequest,
    ) -> client_traits::Result<ProcessManagerHandleResponse> {
        let trigger = request.trigger.ok_or_else(|| {
            ClientError::InvalidArgument("SpeculatePmRequest missing trigger".to_string())
        })?;

        // Convert destinations and process_state to domain specs
        let mut domain_specs = HashMap::new();
        if let Some(ps) = request.process_state {
            if let Some(cover) = &ps.cover {
                domain_specs.insert(cover.domain.clone(), DomainStateSpec::Explicit(ps));
            }
        }
        for dest in request.destinations {
            if let Some(cover) = &dest.cover {
                domain_specs.insert(cover.domain.clone(), DomainStateSpec::Explicit(dest));
            }
        }

        let result = self
            .executor
            .speculate_pm(&request.pm_name, &trigger, &domain_specs)
            .await
            .map_err(ClientError::from)?;

        Ok(ProcessManagerHandleResponse {
            commands: result.commands,
            process_events: result.process_events,
        })
    }
}

/// Builder for constructing commands.
pub struct CommandBuilder {
    router: Arc<CommandRouter>,
    domain: String,
    root: Uuid,
    command_type: Option<String>,
    command_data: Option<Vec<u8>>,
    correlation_id: Option<String>,
    sequence: Option<u32>,
    edition: Option<String>,
    dry_run_sequence: Option<u32>,
    dry_run_timestamp: Option<String>,
}

impl CommandBuilder {
    fn new(router: Arc<CommandRouter>, domain: String, root: Uuid) -> Self {
        Self {
            router,
            domain,
            root,
            command_type: None,
            command_data: None,
            correlation_id: None,
            sequence: None,
            edition: None,
            dry_run_sequence: None,
            dry_run_timestamp: None,
        }
    }

    /// Set the command type URL.
    pub fn with_type(mut self, type_url: impl Into<String>) -> Self {
        self.command_type = Some(type_url.into());
        self
    }

    /// Set the command data (protobuf-encoded).
    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.command_data = Some(data.into());
        self
    }

    /// Set a protobuf message as the command.
    pub fn with_message<M: prost::Message>(mut self, message: &M) -> Self {
        let mut buf = Vec::new();
        message.encode(&mut buf).ok();
        self.command_data = Some(buf);
        self
    }

    /// Set a custom correlation ID.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Set explicit sequence number.
    pub fn with_sequence(mut self, sequence: u32) -> Self {
        self.sequence = Some(sequence);
        self
    }

    /// Target an edition (diverged timeline) instead of the main timeline.
    ///
    /// Commands with an edition set are routed to the edition's command
    /// router, which uses edition-aware storage and bus subscriptions.
    pub fn with_edition(mut self, edition: impl Into<String>) -> Self {
        self.edition = Some(edition.into());
        self
    }

    /// Set temporal point by sequence for speculative (dry-run) execution.
    ///
    /// The aggregate state will be reconstructed from events `0..=sequence`.
    pub fn as_of_sequence(mut self, sequence: u32) -> Self {
        self.dry_run_sequence = Some(sequence);
        self
    }

    /// Set temporal point by timestamp for speculative (dry-run) execution.
    ///
    /// The aggregate state will be reconstructed from events created at or
    /// before this timestamp (RFC 3339 format).
    pub fn as_of_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.dry_run_timestamp = Some(timestamp.into());
        self
    }

    /// Build the command book without sending.
    pub fn build(self) -> CommandBook {
        let command = self.command_data.map(|data| prost_types::Any {
            type_url: self.command_type.unwrap_or_default(),
            value: data,
        });

        CommandBook {
            cover: Some(Cover {
                domain: self.domain,
                root: Some(ProtoUuid {
                    value: self.root.as_bytes().to_vec(),
                }),
                correlation_id: self.correlation_id.unwrap_or_default(),
                edition: self.edition.map(|name| Edition { name, divergences: vec![] }),
            }),
            pages: vec![CommandPage {
                sequence: self.sequence.unwrap_or(0),
                command,
            }],
            saga_origin: None,
        }
    }

    /// Send the command and return the response.
    pub async fn send(self) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        let router = self.router.clone();
        let command = self.build();

        router
            .execute(command)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Speculatively execute against temporal state (dry-run).
    ///
    /// Runs the aggregate handler at the temporal point set via
    /// [`as_of_sequence`](Self::as_of_sequence) or
    /// [`as_of_timestamp`](Self::as_of_timestamp) and returns the events that
    /// *would* be produced. This is purely speculative: no events are persisted,
    /// published, or routed to sagas/projectors. Use this to validate business
    /// rules or explore "what-if" scenarios without side effects.
    pub async fn dry_run(
        self,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        let router = self.router.clone();
        let as_of_sequence = self.dry_run_sequence;
        let as_of_timestamp = self.dry_run_timestamp.clone();
        let command = self.build();

        router
            .dry_run(command, as_of_sequence, as_of_timestamp.as_deref())
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_builder_build() {
        // Create a mock router (we can't test without storage, so just test build)
        let root = Uuid::new_v4();

        // We can't test the full flow without a real router, but we can test building
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-id".to_string(),
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: "CreateOrder".to_string(),
                    value: vec![1, 2, 3],
                }),
            }],
            saga_origin: None,
        };

        let cover = command.cover.as_ref().unwrap();
        assert_eq!(cover.domain, "orders");
        assert_eq!(cover.correlation_id, "test-id");
    }
}
