//! Saga coordinator service.
//!
//! Receives events from the event bus and distributes them to registered sagas.
//! Sagas can produce new commands in response to events, enabling cross-aggregate
//! workflows.

use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::bus::SagaConfig;
use crate::proto::{
    saga_client::SagaClient, saga_coordinator_server::SagaCoordinator, EventBook,
    SynchronousProcessingResponse,
};

/// Re-export for backwards compatibility.
pub type SagaEndpoint = SagaConfig;

/// Connected saga client.
struct SagaConnection {
    config: SagaConfig,
    client: SagaClient<Channel>,
}

/// Saga coordinator service.
///
/// Distributes events to all registered sagas and collects responses.
pub struct SagaCoordinatorService {
    sagas: Arc<RwLock<Vec<SagaConnection>>>,
}

impl SagaCoordinatorService {
    /// Create a new saga coordinator.
    pub fn new() -> Self {
        Self {
            sagas: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a saga endpoint.
    pub async fn add_saga(&self, config: SagaConfig) -> Result<(), String> {
        let channel = Channel::from_shared(format!("http://{}", config.address))
            .map_err(|e| e.to_string())?
            .connect()
            .await
            .map_err(|e| e.to_string())?;

        let client = SagaClient::new(channel);

        info!(
            saga = %config.name,
            address = %config.address,
            "Registered saga"
        );

        self.sagas
            .write()
            .await
            .push(SagaConnection { config, client });

        Ok(())
    }
}

impl Default for SagaCoordinatorService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl SagaCoordinator for SagaCoordinatorService {
    /// Handle events synchronously, collecting all resulting event books.
    async fn handle_sync(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<SynchronousProcessingResponse>, Status> {
        let event_book = request.into_inner();

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let sagas = self.sagas.read().await;
            sagas
                .iter()
                .filter(|conn| conn.config.synchronous)
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        let mut all_books = Vec::new();
        let mut all_projections = Vec::new();

        for (config, mut client) in connections {
            let req = Request::new(event_book.clone());
            match client.handle_sync(req).await {
                Ok(response) => {
                    info!(saga.name = %config.name, "Synchronous saga completed");
                    let inner = response.into_inner();
                    all_books.extend(inner.books);
                    all_projections.extend(inner.projections);
                }
                Err(e) => {
                    error!(saga.name = %config.name, error = %e, "Synchronous saga failed");
                    return Err(Status::internal(format!(
                        "Saga {} failed: {}",
                        config.name, e
                    )));
                }
            }
        }

        Ok(Response::new(SynchronousProcessingResponse {
            books: all_books,
            commands: vec![],
            projections: all_projections,
        }))
    }

    /// Handle events asynchronously (fire and forget).
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let event_book = request.into_inner();

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let sagas = self.sagas.read().await;
            sagas
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        for (config, mut client) in connections {
            let req = Request::new(event_book.clone());

            if config.synchronous {
                match client.handle_sync(req).await {
                    Ok(_) => info!(saga.name = %config.name, "Saga completed"),
                    Err(e) => error!(saga.name = %config.name, error = %e, "Saga failed"),
                }
            } else {
                match client.handle(req).await {
                    Ok(_) => info!(saga.name = %config.name, "Async saga queued"),
                    Err(e) => warn!(saga.name = %config.name, error = %e, "Failed to queue saga"),
                }
            }
        }

        Ok(Response::new(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, Uuid as ProtoUuid};

    fn make_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1; 16],
                }),
            }),
            pages: vec![],
            snapshot: None,
        }
    }

    #[tokio::test]
    async fn test_new_creates_empty_coordinator() {
        let coordinator = SagaCoordinatorService::new();
        assert!(coordinator.sagas.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_default_creates_empty_coordinator() {
        let coordinator = SagaCoordinatorService::default();
        assert!(coordinator.sagas.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_handle_sync_with_no_sagas_returns_empty_response() {
        let coordinator = SagaCoordinatorService::new();
        let event_book = make_event_book();

        let response = coordinator.handle_sync(Request::new(event_book)).await;

        assert!(response.is_ok());
        let sync_response = response.unwrap().into_inner();
        assert!(sync_response.books.is_empty());
        assert!(sync_response.projections.is_empty());
    }

    #[tokio::test]
    async fn test_handle_with_no_sagas_succeeds() {
        let coordinator = SagaCoordinatorService::new();
        let event_book = make_event_book();

        let response = coordinator.handle(Request::new(event_book)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_add_saga_invalid_address() {
        let coordinator = SagaCoordinatorService::new();
        let config = SagaConfig {
            name: "test".to_string(),
            address: "".to_string(), // Invalid
            synchronous: false,
        };

        let result = coordinator.add_saga(config).await;

        assert!(result.is_err());
    }
}
