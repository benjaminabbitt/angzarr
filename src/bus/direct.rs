//! Direct gRPC event bus implementation.
//!
//! This implementation makes synchronous gRPC calls to projectors and sagas
//! rather than using an async message queue.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{error, info, warn};

use crate::interfaces::event_bus::{BusError, EventBus, EventHandler, Result};
use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use crate::proto::saga_coordinator_client::SagaCoordinatorClient;
use crate::proto::EventBook;

// gRPC requires owned EventBook, so we need to clone from Arc for remote calls.
// This is unavoidable for network serialization.

/// Configuration for a projector endpoint.
#[derive(Clone, Debug)]
pub struct ProjectorConfig {
    /// Name of the projector.
    pub name: String,
    /// gRPC address.
    pub address: String,
    /// If true, wait for response before continuing.
    pub synchronous: bool,
}

/// Configuration for a saga endpoint.
#[derive(Clone, Debug)]
pub struct SagaConfig {
    /// Name of the saga.
    pub name: String,
    /// gRPC address.
    pub address: String,
    /// If true, wait for response before continuing.
    pub synchronous: bool,
}

/// Connected projector client.
struct ProjectorConnection {
    config: ProjectorConfig,
    client: ProjectorCoordinatorClient<Channel>,
}

/// Connected saga client.
struct SagaConnection {
    config: SagaConfig,
    client: SagaCoordinatorClient<Channel>,
}

/// Direct gRPC event bus.
///
/// Calls projector and saga services directly via gRPC.
/// For production, consider using AMQP or Kafka for async delivery.
pub struct DirectEventBus {
    projectors: Arc<RwLock<Vec<ProjectorConnection>>>,
    sagas: Arc<RwLock<Vec<SagaConnection>>>,
}

impl DirectEventBus {
    /// Create a new direct event bus.
    pub fn new() -> Self {
        Self {
            projectors: Arc::new(RwLock::new(Vec::new())),
            sagas: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a projector endpoint.
    pub async fn add_projector(&self, config: ProjectorConfig) -> Result<()> {
        let channel = Channel::from_shared(format!("http://{}", config.address))
            .map_err(|e| BusError::Connection(e.to_string()))?
            .connect()
            .await
            .map_err(|e| BusError::Connection(e.to_string()))?;

        let client = ProjectorCoordinatorClient::new(channel);

        info!(
            projector = %config.name,
            address = %config.address,
            "Connected to projector coordinator"
        );

        self.projectors
            .write()
            .await
            .push(ProjectorConnection { config, client });

        Ok(())
    }

    /// Add a saga endpoint.
    pub async fn add_saga(&self, config: SagaConfig) -> Result<()> {
        let channel = Channel::from_shared(format!("http://{}", config.address))
            .map_err(|e| BusError::Connection(e.to_string()))?
            .connect()
            .await
            .map_err(|e| BusError::Connection(e.to_string()))?;

        let client = SagaCoordinatorClient::new(channel);

        info!(
            saga = %config.name,
            address = %config.address,
            "Connected to saga coordinator"
        );

        self.sagas
            .write()
            .await
            .push(SagaConnection { config, client });

        Ok(())
    }

    /// Publish to all projectors.
    async fn publish_to_projectors(&self, book: &Arc<EventBook>) -> Result<()> {
        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let projectors = self.projectors.read().await;
            projectors
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        for (config, mut client) in connections {
            // gRPC requires owned data for serialization
            let request = tonic::Request::new((**book).clone());

            if config.synchronous {
                match client.handle_sync(request).await {
                    Ok(response) => {
                        info!(projector.name = %config.name, "Synchronous projection completed");
                        let _ = response.into_inner();
                    }
                    Err(e) => {
                        error!(projector.name = %config.name, error = %e, "Synchronous projector failed");
                        return Err(e.into());
                    }
                }
            } else {
                match client.handle(request).await {
                    Ok(_) => {
                        info!(projector.name = %config.name, "Async projection queued");
                    }
                    Err(e) => {
                        warn!(projector.name = %config.name, error = %e, "Failed to queue async projection");
                    }
                }
            }
        }

        Ok(())
    }

    /// Publish to all sagas.
    async fn publish_to_sagas(&self, book: &Arc<EventBook>) -> Result<()> {
        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let sagas = self.sagas.read().await;
            sagas
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        for (config, mut client) in connections {
            // gRPC requires owned data for serialization
            let request = tonic::Request::new((**book).clone());

            if config.synchronous {
                match client.handle_sync(request).await {
                    Ok(response) => {
                        info!(saga.name = %config.name, "Synchronous saga completed");
                        let _ = response.into_inner();
                    }
                    Err(e) => {
                        error!(saga.name = %config.name, error = %e, "Synchronous saga failed");
                        return Err(e.into());
                    }
                }
            } else {
                match client.handle(request).await {
                    Ok(_) => {
                        info!(saga.name = %config.name, "Async saga queued");
                    }
                    Err(e) => {
                        warn!(saga.name = %config.name, error = %e, "Failed to queue async saga");
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for DirectEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for DirectEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<()> {
        // Publish to projectors first
        self.publish_to_projectors(&book).await?;

        // Then publish to sagas
        self.publish_to_sagas(&book).await?;

        Ok(())
    }

    async fn subscribe(&self, _handler: Box<dyn EventHandler>) -> Result<()> {
        Err(BusError::SubscribeNotSupported)
    }
}
