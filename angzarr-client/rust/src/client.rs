//! Default client implementations wrapping tonic gRPC clients.

use crate::error::{ClientError, Result};
use crate::traits;
use angzarr::proto::{
    command_gateway_client::CommandGatewayClient as TonicGatewayClient,
    event_query_client::EventQueryClient as TonicQueryClient, CommandBook, CommandResponse,
    DryRunRequest, EventBook, Query,
};
use async_trait::async_trait;
use tonic::transport::Channel;

/// Default gateway client using tonic gRPC.
#[derive(Clone)]
pub struct GatewayClient {
    inner: TonicGatewayClient<Channel>,
}

impl GatewayClient {
    /// Connect to a gateway server at the given endpoint.
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
            inner: TonicGatewayClient::new(channel),
        }
    }
}

#[async_trait]
impl traits::GatewayClient for GatewayClient {
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse> {
        let response = self.inner.clone().execute(command).await?;
        Ok(response.into_inner())
    }

    async fn dry_run(&self, request: DryRunRequest) -> Result<CommandResponse> {
        let response = self.inner.clone().dry_run_execute(request).await?;
        Ok(response.into_inner())
    }
}

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

/// Combined client providing both gateway and query operations.
#[derive(Clone)]
pub struct Client {
    /// Gateway client for command execution.
    pub gateway: GatewayClient,
    /// Query client for event retrieval.
    pub query: QueryClient,
}

impl Client {
    /// Connect to a server providing both gateway and query services.
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
            gateway: GatewayClient::from_channel(channel.clone()),
            query: QueryClient::from_channel(channel),
        }
    }
}
