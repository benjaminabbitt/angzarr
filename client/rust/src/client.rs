//! Default client implementations wrapping tonic gRPC clients.

use std::time::Duration;

use crate::error::{ClientError, Result};
use crate::proto::{
    command_handler_coordinator_service_client::CommandHandlerCoordinatorServiceClient as TonicCommandHandlerClient,
    event_query_service_client::EventQueryServiceClient as TonicQueryClient,
    process_manager_coordinator_service_client::ProcessManagerCoordinatorServiceClient as TonicPmClient,
    projector_coordinator_service_client::ProjectorCoordinatorServiceClient as TonicProjectorClient,
    saga_coordinator_service_client::SagaCoordinatorServiceClient as TonicSagaClient,
    CascadeErrorMode, CommandBook, CommandRequest, CommandResponse, EventBook,
    ProcessManagerHandleResponse, Projection, Query, SagaResponse, SpeculateCommandHandlerRequest,
    SpeculatePmRequest, SpeculateProjectorRequest, SpeculateSagaRequest, SyncMode,
};
use crate::traits;
use async_trait::async_trait;
use backon::{BackoffBuilder, ExponentialBuilder};
use tonic::transport::{Channel, Endpoint, Uri};
use tracing::warn;

/// Create a gRPC channel from an endpoint string.
///
/// Supports both TCP (host:port or http://host:port) and Unix Domain Sockets.
/// UDS paths are detected by leading '/' or './' and use a custom connector.
///
/// Retries connection with exponential backoff (100ms-5s, 10 attempts) on failure.
async fn create_channel(endpoint: &str) -> Result<Channel> {
    let uds_path = if endpoint.starts_with('/') || endpoint.starts_with("./") {
        Some(endpoint.to_string())
    } else {
        endpoint.strip_prefix("unix://").map(str::to_string)
    };

    let backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(100))
        .with_max_delay(Duration::from_secs(5))
        .with_max_times(10)
        .with_jitter()
        .build();

    let mut last_error: Option<ClientError> = None;

    for (attempt, delay) in std::iter::once(Duration::ZERO).chain(backoff).enumerate() {
        if attempt > 0 {
            warn!(
                endpoint = %endpoint,
                attempt = attempt,
                backoff_ms = %delay.as_millis(),
                "gRPC connection failed, retrying after backoff"
            );
            tokio::time::sleep(delay).await;
        }

        let result = if let Some(ref path) = uds_path {
            // Unix Domain Socket - use custom connector
            // NOTE: The URI is ignored for UDS, but tonic requires a valid one.
            // We use a dummy URI and override the connector to use UnixStream.
            let path = path.clone();
            Endpoint::try_from("http://[::]:50051")
                .map_err(|e| ClientError::Connection { msg: e.to_string() })?
                .connect_with_connector(tower::service_fn(move |_: Uri| {
                    let path = path.clone();
                    async move {
                        tokio::net::UnixStream::connect(path)
                            .await
                            .map(hyper_util::rt::TokioIo::new)
                    }
                }))
                .await
        } else {
            // TCP endpoint
            match Channel::from_shared(endpoint.to_string()) {
                Ok(ep) => ep.connect().await,
                Err(e) => {
                    // Invalid URI is not retryable
                    return Err(ClientError::Connection { msg: e.to_string() });
                }
            }
        };

        match result {
            Ok(channel) => return Ok(channel),
            Err(e) => {
                last_error = Some(ClientError::Connection {
                    msg: format!("Connection failed: {}", e),
                });
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ClientError::Connection {
        msg: "Connection failed after max retries".to_string(),
    }))
}

/// Default event query client using tonic gRPC.
#[derive(Clone)]
pub struct QueryClient {
    inner: TonicQueryClient<Channel>,
}

impl QueryClient {
    /// Connect to an event query service at the given endpoint.
    ///
    /// Supports both TCP (host:port) and Unix Domain Sockets (file paths).
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = create_channel(endpoint).await?;
        Ok(Self::from_channel(channel))
    }

    /// Connect using an endpoint from environment variable with fallback.
    pub async fn from_env(env_var: &str, default: &str) -> Result<Self> {
        let endpoint = std::env::var(env_var).unwrap_or_else(|_| default.to_string());
        Self::connect(&endpoint).await
    }

    /// Create a client from an existing channel.
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            inner: TonicQueryClient::new(channel),
        }
    }

    /// Query events for an aggregate.
    pub async fn get_events(&self, query: Query) -> Result<EventBook> {
        let response = self.inner.clone().get_event_book(query).await?;
        Ok(response.into_inner())
    }
}

#[async_trait]
impl traits::QueryClient for QueryClient {
    async fn get_events(&self, query: Query) -> Result<EventBook> {
        self.get_events(query).await
    }
}

/// Default command handler coordinator client using tonic gRPC.
#[derive(Clone)]
pub struct CommandHandlerClient {
    inner: TonicCommandHandlerClient<Channel>,
}

impl CommandHandlerClient {
    /// Connect to a command handler coordinator at the given endpoint.
    ///
    /// Supports both TCP (host:port) and Unix Domain Sockets (file paths).
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = create_channel(endpoint).await?;
        Ok(Self::from_channel(channel))
    }

    /// Connect using an endpoint from environment variable with fallback.
    pub async fn from_env(env_var: &str, default: &str) -> Result<Self> {
        let endpoint = std::env::var(env_var).unwrap_or_else(|_| default.to_string());
        Self::connect(&endpoint).await
    }

    /// Create a client from an existing channel.
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            inner: TonicCommandHandlerClient::new(channel),
        }
    }

    /// Execute a command with specified sync mode.
    ///
    /// Use `SyncMode::Async` for fire-and-forget (default).
    /// Use `SyncMode::Simple` to wait for sync projectors.
    /// Use `SyncMode::Cascade` for full sync including saga cascade.
    pub async fn handle_command(&self, command: CommandRequest) -> Result<CommandResponse> {
        let response = self.inner.clone().handle_command(command).await?;
        Ok(response.into_inner())
    }

    /// Execute a command asynchronously (fire-and-forget).
    ///
    /// Convenience method that wraps CommandBook in CommandRequest with async sync mode.
    pub async fn handle(&self, command: CommandBook) -> Result<CommandResponse> {
        self.handle_command(CommandRequest {
            command: Some(command),
            sync_mode: SyncMode::Async as i32,
            cascade_error_mode: CascadeErrorMode::CascadeErrorFailFast as i32,
        })
        .await
    }

    /// Speculative execution against temporal state.
    pub async fn handle_sync_speculative(
        &self,
        request: SpeculateCommandHandlerRequest,
    ) -> Result<CommandResponse> {
        let response = self.inner.clone().handle_sync_speculative(request).await?;
        Ok(response.into_inner())
    }
}

#[async_trait]
impl traits::GatewayClient for CommandHandlerClient {
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.handle(command).await
    }
}

/// Per-domain client combining command execution, event querying, and speculative operations.
///
/// Connects to a single domain's endpoint and provides:
/// - Command execution via `command_handler`
/// - Event querying via `query`
/// - Speculative (what-if) execution via `speculative`
///
/// Matches the distributed architecture where each domain has its own coordinator service.
#[derive(Clone)]
pub struct DomainClient {
    /// Command handler client for command execution.
    pub command_handler: CommandHandlerClient,
    /// Query client for event retrieval.
    pub query: QueryClient,
    /// Speculative client for dry-run and what-if scenarios.
    pub speculative: SpeculativeClient,
}

impl DomainClient {
    /// Connect to a domain's coordinator at the given endpoint.
    ///
    /// Supports both TCP (host:port) and Unix Domain Sockets (file paths).
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = create_channel(endpoint).await?;
        Ok(Self::from_channel(channel))
    }

    /// Connect using an endpoint from environment variable with fallback.
    pub async fn from_env(env_var: &str, default: &str) -> Result<Self> {
        let endpoint = std::env::var(env_var).unwrap_or_else(|_| default.to_string());
        Self::connect(&endpoint).await
    }

    /// Create a client from an existing channel.
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            command_handler: CommandHandlerClient::from_channel(channel.clone()),
            query: QueryClient::from_channel(channel.clone()),
            speculative: SpeculativeClient::from_channel(channel),
        }
    }

    /// Execute a command asynchronously (fire-and-forget).
    ///
    /// Use `execute_with_mode()` to specify a different sync mode.
    pub async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.command_handler.handle(command).await
    }

    /// Execute a command with the specified sync mode.
    ///
    /// Use `SyncMode::Async` for fire-and-forget (default).
    /// Use `SyncMode::Simple` to wait for sync projectors.
    /// Use `SyncMode::Cascade` for full sync including saga cascade.
    pub async fn execute_with_mode(
        &self,
        command: CommandBook,
        sync_mode: SyncMode,
    ) -> Result<CommandResponse> {
        self.command_handler
            .handle_command(CommandRequest {
                command: Some(command),
                sync_mode: sync_mode as i32,
                cascade_error_mode: CascadeErrorMode::CascadeErrorFailFast as i32,
            })
            .await
    }

    /// Query events (delegates to query client).
    pub async fn get_events(&self, query: Query) -> Result<EventBook> {
        self.query.get_events(query).await
    }
}

#[async_trait]
impl traits::GatewayClient for DomainClient {
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.execute(command).await
    }
}

#[async_trait]
impl traits::QueryClient for DomainClient {
    async fn get_events(&self, query: Query) -> Result<EventBook> {
        self.get_events(query).await
    }
}

/// Speculative client for what-if scenarios.
///
/// Provides speculative execution across different coordinator types.
/// Each method targets a specific coordinator's speculative RPC.
#[derive(Clone)]
pub struct SpeculativeClient {
    command_handler: TonicCommandHandlerClient<Channel>,
    projector: TonicProjectorClient<Channel>,
    saga: TonicSagaClient<Channel>,
    pm: TonicPmClient<Channel>,
}

impl SpeculativeClient {
    /// Connect to services at the given endpoint.
    ///
    /// Supports both TCP (host:port) and Unix Domain Sockets (file paths).
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = create_channel(endpoint).await?;
        Ok(Self::from_channel(channel))
    }

    /// Connect using an endpoint from environment variable with fallback.
    pub async fn from_env(env_var: &str, default: &str) -> Result<Self> {
        let endpoint = std::env::var(env_var).unwrap_or_else(|_| default.to_string());
        Self::connect(&endpoint).await
    }

    /// Create a client from an existing channel.
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            command_handler: TonicCommandHandlerClient::new(channel.clone()),
            projector: TonicProjectorClient::new(channel.clone()),
            saga: TonicSagaClient::new(channel.clone()),
            pm: TonicPmClient::new(channel),
        }
    }
}

#[async_trait]
impl traits::SpeculativeClient for SpeculativeClient {
    async fn command_handler(
        &self,
        request: SpeculateCommandHandlerRequest,
    ) -> Result<CommandResponse> {
        let response = self
            .command_handler
            .clone()
            .handle_sync_speculative(request)
            .await?;
        Ok(response.into_inner())
    }

    async fn projector(&self, request: SpeculateProjectorRequest) -> Result<Projection> {
        let response = self.projector.clone().handle_speculative(request).await?;
        Ok(response.into_inner())
    }

    async fn saga(&self, request: SpeculateSagaRequest) -> Result<SagaResponse> {
        let response = self.saga.clone().execute_speculative(request).await?;
        Ok(response.into_inner())
    }

    async fn process_manager(
        &self,
        request: SpeculatePmRequest,
    ) -> Result<ProcessManagerHandleResponse> {
        let response = self.pm.clone().handle_speculative(request).await?;
        Ok(response.into_inner())
    }
}
