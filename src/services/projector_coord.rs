//! Projector coordinator service.
//!
//! Receives events from the event bus and distributes them to registered projectors.
//! Ensures projectors receive complete EventBooks by fetching missing history
//! from the EventQuery service when needed.

use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::config::ServiceEndpoint;
use crate::grpc::connect_channel;
use crate::proto::{
    projector_client::ProjectorClient, projector_coordinator_server::ProjectorCoordinator,
    EventBook, Projection, SyncEventBook,
};
use crate::proto_ext::{correlated_request, CoverExt};
use crate::services::event_book_repair::EventBookRepairer;

/// Connected projector client.
struct ProjectorConnection {
    config: ServiceEndpoint,
    client: ProjectorClient<Channel>,
}

/// Projector coordinator service.
///
/// Distributes events to all registered projectors. Before forwarding,
/// checks if EventBooks are complete and fetches missing history from
/// the EventQuery service if needed.
pub struct ProjectorCoordinatorService {
    projectors: Arc<RwLock<Vec<ProjectorConnection>>>,
    repairer: Arc<Mutex<EventBookRepairer>>,
}

impl ProjectorCoordinatorService {
    /// Create a new projector coordinator.
    pub fn new(repairer: EventBookRepairer) -> Self {
        Self {
            projectors: Arc::new(RwLock::new(Vec::new())),
            repairer: Arc::new(Mutex::new(repairer)),
        }
    }

    /// Create a new projector coordinator, connecting to EventQuery service.
    pub async fn connect(event_query_address: &str) -> Result<Self, String> {
        let repairer = EventBookRepairer::connect(event_query_address)
            .await
            .map_err(|e| format!("Failed to connect to EventQuery service: {}", e))?;

        info!(
            address = %event_query_address,
            "Connected to EventQuery service for EventBook repair"
        );

        Ok(Self::new(repairer))
    }

    /// Register a projector endpoint.
    pub async fn add_projector(&self, config: ServiceEndpoint) -> Result<(), String> {
        let channel = connect_channel(&config.address).await?;
        let client = ProjectorClient::new(channel);

        info!(
            projector = %config.name,
            address = %config.address,
            "Registered projector"
        );

        self.projectors
            .write()
            .await
            .push(ProjectorConnection { config, client });

        Ok(())
    }
}

#[tonic::async_trait]
impl ProjectorCoordinator for ProjectorCoordinatorService {
    /// Handle events synchronously, returning a projection.
    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<Projection>, Status> {
        let sync_request = request.into_inner();
        let event_book = sync_request
            .events
            .ok_or_else(|| Status::invalid_argument("SyncEventBook must have events"))?;

        // Repair EventBook if incomplete
        let event_book = self
            .repairer
            .lock()
            .await
            .repair(event_book)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to repair EventBook");
                Status::internal(format!("Failed to repair EventBook: {}", e))
            })?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let projectors = self.projectors.read().await;
            projectors
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        // Return the first successful projection
        let correlation_id = event_book.correlation_id().to_string();
        if let Some((config, mut client)) = connections.into_iter().next() {
            let req = correlated_request(event_book.clone(), &correlation_id);
            match client.handle(req).await {
                Ok(response) => {
                    info!(projector.name = %config.name, "Synchronous projection completed");
                    return Ok(response);
                }
                Err(e) => {
                    error!(projector.name = %config.name, error = %e, "Synchronous projector failed");
                    return Err(Status::internal(format!(
                        "Projector {} failed: {}",
                        config.name, e
                    )));
                }
            }
        }

        // No synchronous projectors, return empty projection
        let cover = event_book.cover.clone();
        Ok(Response::new(Projection {
            cover,
            projector: String::new(),
            sequence: 0,
            projection: None,
        }))
    }

    /// Handle events asynchronously (fire and forget).
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let event_book = request.into_inner();

        // Repair EventBook if incomplete
        let event_book = self
            .repairer
            .lock()
            .await
            .repair(event_book)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to repair EventBook");
                Status::internal(format!("Failed to repair EventBook: {}", e))
            })?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let projectors = self.projectors.read().await;
            projectors
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        let correlation_id = event_book.correlation_id().to_string();
        for (config, mut client) in connections {
            let req = correlated_request(event_book.clone(), &correlation_id);
            match client.handle(req).await {
                Ok(_) => info!(projector.name = %config.name, "Async projection queued"),
                Err(e) => {
                    warn!(projector.name = %config.name, error = %e, "Failed to queue projection")
                }
            }
        }

        Ok(Response::new(()))
    }

    /// Handle events speculatively - returns projection without side effects.
    ///
    /// Same as handle_sync but explicitly for speculative execution.
    async fn handle_speculative(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();

        // Repair EventBook if incomplete
        let event_book = self
            .repairer
            .lock()
            .await
            .repair(event_book)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to repair EventBook");
                Status::internal(format!("Failed to repair EventBook: {}", e))
            })?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let projectors = self.projectors.read().await;
            projectors
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        // Return the first successful projection
        let correlation_id = event_book.correlation_id().to_string();
        if let Some((config, mut client)) = connections.into_iter().next() {
            let req = correlated_request(event_book.clone(), &correlation_id);
            match client.handle(req).await {
                Ok(response) => {
                    info!(projector.name = %config.name, "Speculative projection completed");
                    return Ok(response);
                }
                Err(e) => {
                    error!(projector.name = %config.name, error = %e, "Speculative projector failed");
                    return Err(Status::internal(format!(
                        "Projector {} failed: {}",
                        config.name, e
                    )));
                }
            }
        }

        // No projectors, return empty projection
        let cover = event_book.cover.clone();
        Ok(Response::new(Projection {
            cover,
            projector: String::new(),
            sequence: 0,
            projection: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::event_query_server::EventQueryServer;
    use crate::proto::{Cover, Uuid as ProtoUuid};
    use crate::services::EventQueryService;
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tonic::transport::Server;

    fn make_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        }
    }

    async fn start_event_query_server() -> SocketAddr {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let service = EventQueryService::new(event_store, snapshot_store);

        tokio::spawn(async move {
            Server::builder()
                .add_service(EventQueryServer::new(service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        addr
    }

    #[tokio::test]
    async fn test_handle_sync_with_no_projectors_returns_empty_projection() {
        let addr = start_event_query_server().await;
        let coordinator = ProjectorCoordinatorService::connect(&addr.to_string())
            .await
            .unwrap();

        let event_book = make_event_book();
        let sync_request = SyncEventBook {
            events: Some(event_book),
            sync_mode: crate::proto::SyncMode::Simple.into(),
        };

        let response = coordinator.handle_sync(Request::new(sync_request)).await;

        assert!(response.is_ok());
        let projection = response.unwrap().into_inner();
        assert!(projection.projector.is_empty());
        assert_eq!(projection.sequence, 0);
    }

    #[tokio::test]
    async fn test_handle_with_no_projectors_succeeds() {
        let addr = start_event_query_server().await;
        let coordinator = ProjectorCoordinatorService::connect(&addr.to_string())
            .await
            .unwrap();

        let event_book = make_event_book();

        let response = coordinator.handle(Request::new(event_book)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_add_projector_invalid_address() {
        let addr = start_event_query_server().await;
        let coordinator = ProjectorCoordinatorService::connect(&addr.to_string())
            .await
            .unwrap();

        let config = ServiceEndpoint {
            name: "test".to_string(),
            address: "".to_string(), // Invalid
        };

        let result = coordinator.add_projector(config).await;

        assert!(result.is_err());
    }
}
