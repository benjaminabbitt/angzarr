//! Default client implementations wrapping tonic gRPC clients.

use crate::error::{ClientError, Result};
use crate::traits;
use angzarr::proto::{
    aggregate_coordinator_client::AggregateCoordinatorClient as TonicAggregateClient,
    event_query_client::EventQueryClient as TonicQueryClient,
    speculative_service_client::SpeculativeServiceClient as TonicSpeculativeClient, CommandBook,
    CommandResponse, DryRunRequest, EventBook, ProcessManagerHandleResponse, Projection, Query,
    SagaResponse, SpeculatePmRequest, SpeculateProjectorRequest, SpeculateSagaRequest,
    SyncCommandBook,
};
use async_trait::async_trait;
use tonic::transport::Channel;

/// Default query client using tonic gRPC.
#[derive(Clone)]
pub struct QueryClient {
    inner: TonicQueryClient<Channel>,
}

impl QueryClient {
    /// Connect to a query server at the given endpoint.
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect()
            .await?;

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
}

#[async_trait]
impl traits::QueryClient for QueryClient {
    async fn get_event_book(&self, query: Query) -> Result<EventBook> {
        let response = self.inner.clone().get_event_book(query).await?;
        Ok(response.into_inner())
    }

    async fn get_events(&self, query: Query) -> Result<Vec<EventBook>> {
        let mut stream = self.inner.clone().get_events(query).await?.into_inner();
        let mut events = Vec::new();

        while let Some(event_book) = stream.message().await? {
            events.push(event_book);
        }

        Ok(events)
    }
}

/// Per-domain aggregate coordinator client using tonic gRPC.
///
/// Connects directly to a domain's aggregate coordinator (AggregateCoordinator service).
/// Use this when connecting to per-domain endpoints rather than a central gateway.
#[derive(Clone)]
pub struct AggregateClient {
    inner: TonicAggregateClient<Channel>,
}

impl AggregateClient {
    /// Connect to an aggregate coordinator at the given endpoint.
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect()
            .await?;

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

    /// Dry-run a command against temporal state.
    pub async fn dry_run_handle(&self, request: DryRunRequest) -> Result<CommandResponse> {
        let response = self.inner.clone().dry_run_handle(request).await?;
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
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect()
            .await?;

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

/// Default speculative client using tonic gRPC.
#[derive(Clone)]
pub struct SpeculativeClient {
    inner: TonicSpeculativeClient<Channel>,
}

impl SpeculativeClient {
    /// Connect to a speculative service at the given endpoint.
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect()
            .await?;

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
            inner: TonicSpeculativeClient::new(channel),
        }
    }
}

#[async_trait]
impl traits::SpeculativeClient for SpeculativeClient {
    async fn dry_run(&self, request: DryRunRequest) -> Result<CommandResponse> {
        let response = self.inner.clone().dry_run_command(request).await?;
        Ok(response.into_inner())
    }

    async fn projector(&self, request: SpeculateProjectorRequest) -> Result<Projection> {
        let response = self.inner.clone().speculate_projector(request).await?;
        Ok(response.into_inner())
    }

    async fn saga(&self, request: SpeculateSagaRequest) -> Result<SagaResponse> {
        let response = self.inner.clone().speculate_saga(request).await?;
        Ok(response.into_inner())
    }

    async fn process_manager(
        &self,
        request: SpeculatePmRequest,
    ) -> Result<ProcessManagerHandleResponse> {
        let response = self
            .inner
            .clone()
            .speculate_process_manager(request)
            .await?;
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
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| ClientError::Connection(e.to_string()))?
            .connect()
            .await?;

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
}
