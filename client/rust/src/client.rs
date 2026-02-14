//! Default client implementations wrapping tonic gRPC clients.

use crate::error::{ClientError, Result};
use crate::proto::{
    aggregate_coordinator_service_client::AggregateCoordinatorServiceClient as TonicAggregateClient,
    event_query_service_client::EventQueryServiceClient as TonicQueryClient,
    process_manager_coordinator_service_client::ProcessManagerCoordinatorServiceClient as TonicPmClient,
    projector_coordinator_service_client::ProjectorCoordinatorServiceClient as TonicProjectorClient,
    saga_coordinator_service_client::SagaCoordinatorServiceClient as TonicSagaClient, CommandBook,
    CommandResponse, EventBook, ProcessManagerHandleResponse, Projection, Query, SagaResponse,
    SpeculateAggregateRequest, SpeculatePmRequest, SpeculateProjectorRequest, SpeculateSagaRequest,
    SyncCommandBook,
};
use crate::traits;
use async_trait::async_trait;
use tonic::transport::{Channel, Endpoint, Uri};

/// Create a gRPC channel from an endpoint string.
///
/// Supports both TCP (host:port or http://host:port) and Unix Domain Sockets.
/// UDS paths are detected by leading '/' or './' and use a custom connector.
async fn create_channel(endpoint: &str) -> Result<Channel> {
    let uds_path = if endpoint.starts_with('/') || endpoint.starts_with("./") {
        Some(endpoint.to_string())
    } else {
        endpoint.strip_prefix("unix://").map(str::to_string)
    };

    if let Some(path) = uds_path {
        // Unix Domain Socket - use custom connector
        // The URI doesn't matter for UDS, but tonic requires a valid one
        let channel = Endpoint::try_from("http://[::]:50051")
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect_with_connector(tower::service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    tokio::net::UnixStream::connect(path)
                        .await
                        .map(hyper_util::rt::TokioIo::new)
                }
            }))
            .await?;
        Ok(channel)
    } else {
        // TCP endpoint
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect()
            .await?;
        Ok(channel)
    }
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

/// Default aggregate coordinator client using tonic gRPC.
#[derive(Clone)]
pub struct AggregateClient {
    inner: TonicAggregateClient<Channel>,
}

impl AggregateClient {
    /// Connect to an aggregate coordinator at the given endpoint.
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
            inner: TonicAggregateClient::new(channel),
        }
    }

    /// Execute a command asynchronously.
    pub async fn handle(&self, command: CommandBook) -> Result<CommandResponse> {
        let response = self.inner.clone().handle(command).await?;
        Ok(response.into_inner())
    }

    /// Execute a command synchronously with specified sync mode.
    pub async fn handle_sync(&self, command: SyncCommandBook) -> Result<CommandResponse> {
        let response = self.inner.clone().handle_sync(command).await?;
        Ok(response.into_inner())
    }

    /// Speculative execution against temporal state.
    pub async fn handle_sync_speculative(
        &self,
        request: SpeculateAggregateRequest,
    ) -> Result<CommandResponse> {
        let response = self.inner.clone().handle_sync_speculative(request).await?;
        Ok(response.into_inner())
    }
}

#[async_trait]
impl traits::GatewayClient for AggregateClient {
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.handle(command).await
    }
}

/// Per-domain client combining aggregate coordinator and event query.
///
/// Connects to a single domain's endpoint and provides both command execution
/// and event querying. Matches the distributed architecture where each domain
/// has its own coordinator service.
#[derive(Clone)]
pub struct DomainClient {
    /// Aggregate client for command execution.
    pub aggregate: AggregateClient,
    /// Query client for event retrieval.
    pub query: QueryClient,
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
            aggregate: AggregateClient::from_channel(channel.clone()),
            query: QueryClient::from_channel(channel),
        }
    }

    /// Execute a command (delegates to aggregate client).
    pub async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.aggregate.handle(command).await
    }
}

/// Speculative client for what-if scenarios.
///
/// Provides speculative execution across different coordinator types.
/// Each method targets a specific coordinator's speculative RPC.
#[derive(Clone)]
pub struct SpeculativeClient {
    aggregate: TonicAggregateClient<Channel>,
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
            aggregate: TonicAggregateClient::new(channel.clone()),
            projector: TonicProjectorClient::new(channel.clone()),
            saga: TonicSagaClient::new(channel.clone()),
            pm: TonicPmClient::new(channel),
        }
    }
}

#[async_trait]
impl traits::SpeculativeClient for SpeculativeClient {
    async fn aggregate(&self, request: SpeculateAggregateRequest) -> Result<CommandResponse> {
        let response = self
            .aggregate
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

/// Combined client providing aggregate, query, and speculative operations.
#[derive(Clone)]
pub struct Client {
    /// Aggregate client for command execution.
    pub aggregate: AggregateClient,
    /// Query client for event retrieval.
    pub query: QueryClient,
    /// Speculative client for dry-run and what-if scenarios.
    pub speculative: SpeculativeClient,
}

impl Client {
    /// Connect to a server providing aggregate, query, and speculative services.
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
            aggregate: AggregateClient::from_channel(channel.clone()),
            query: QueryClient::from_channel(channel.clone()),
            speculative: SpeculativeClient::from_channel(channel),
        }
    }

    /// Execute a command (delegates to aggregate client).
    pub async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.aggregate.handle(command).await
    }

    /// Query events (delegates to query client).
    pub async fn get_events(&self, query: Query) -> Result<EventBook> {
        self.query.get_events(query).await
    }
}

#[async_trait]
impl traits::GatewayClient for Client {
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        self.execute(command).await
    }
}

#[async_trait]
impl traits::QueryClient for Client {
    async fn get_events(&self, query: Query) -> Result<EventBook> {
        self.get_events(query).await
    }
}
