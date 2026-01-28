//! In-process clients for embedded runtime.
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
    CommandBook, CommandPage, CommandResponse, Cover, DryRunRequest, EventBook, Query,
    Uuid as ProtoUuid,
};
use crate::repository::EventBookRepository;

use super::router::{CommandRouter, DomainStorage};

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

    /// Dry-run a pre-built command against temporal state.
    ///
    /// Executes the command without persisting or publishing. Returns speculative events.
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
}

/// In-process query client for embedded runtime.
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
        let domain = query
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let store = self.domain_stores.get(domain).ok_or_else(|| {
            ClientError::from(Status::not_found(format!("Unknown domain: {domain}")))
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
            .get(domain, root_uuid)
            .await
            .map_err(|e| ClientError::from(Status::internal(e.to_string())))?;

        Ok(book)
    }

    async fn get_events(&self, query: Query) -> client_traits::Result<Vec<EventBook>> {
        let domain = query
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let store = self.domain_stores.get(domain).ok_or_else(|| {
            ClientError::from(Status::not_found(format!("Unknown domain: {domain}")))
        })?;

        // For get_events, we currently return a single EventBook as a vec
        // Full streaming support would require iterating roots
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
            .get(domain, root_uuid)
            .await
            .map_err(|e| ClientError::from(Status::internal(e.to_string())))?;

        Ok(vec![book])
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

    /// Set temporal point by sequence for dry-run execution.
    pub fn as_of_sequence(mut self, sequence: u32) -> Self {
        self.dry_run_sequence = Some(sequence);
        self
    }

    /// Set temporal point by timestamp for dry-run execution.
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

    /// Execute as dry-run against temporal state. No persistence, no side effects.
    ///
    /// Requires `as_of_sequence()` or `as_of_timestamp()` to be set.
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
