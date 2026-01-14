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

use crate::bus::ProjectorConfig;
use crate::proto::{
    projector_client::ProjectorClient, projector_coordinator_server::ProjectorCoordinator,
    EventBook, Projection,
};
use crate::services::event_book_repair::{self, EventBookRepairer};

/// Re-export for backwards compatibility.
pub type ProjectorEndpoint = ProjectorConfig;

/// Connected projector client.
struct ProjectorConnection {
    config: ProjectorConfig,
    client: ProjectorClient<Channel>,
}

/// Projector coordinator service.
///
/// Distributes events to all registered projectors. Before forwarding,
/// checks if EventBooks are complete and fetches missing history from
/// the EventQuery service if needed.
pub struct ProjectorCoordinatorService {
    projectors: Arc<RwLock<Vec<ProjectorConnection>>>,
    repairer: Option<Arc<Mutex<EventBookRepairer>>>,
}

impl ProjectorCoordinatorService {
    /// Create a new projector coordinator without repair capability.
    pub fn new() -> Self {
        Self {
            projectors: Arc::new(RwLock::new(Vec::new())),
            repairer: None,
        }
    }

    /// Create a new projector coordinator with repair capability.
    ///
    /// The repairer will fetch missing events from the EventQuery service
    /// at the given address when incomplete EventBooks are received.
    pub async fn with_repair(event_query_address: &str) -> Result<Self, String> {
        let repairer = EventBookRepairer::connect(event_query_address)
            .await
            .map_err(|e| format!("Failed to connect to EventQuery service: {}", e))?;

        info!(
            address = %event_query_address,
            "Connected to EventQuery service for EventBook repair"
        );

        Ok(Self {
            projectors: Arc::new(RwLock::new(Vec::new())),
            repairer: Some(Arc::new(Mutex::new(repairer))),
        })
    }

    /// Create a new projector coordinator with an existing repairer.
    pub fn with_repairer(repairer: EventBookRepairer) -> Self {
        Self {
            projectors: Arc::new(RwLock::new(Vec::new())),
            repairer: Some(Arc::new(Mutex::new(repairer))),
        }
    }

    /// Register a projector endpoint.
    pub async fn add_projector(&self, config: ProjectorConfig) -> Result<(), String> {
        let channel = Channel::from_shared(format!("http://{}", config.address))
            .map_err(|e| e.to_string())?
            .connect()
            .await
            .map_err(|e| e.to_string())?;

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

    /// Repair an EventBook if incomplete.
    ///
    /// If a repairer is configured and the EventBook is incomplete,
    /// fetches the complete history from the EventQuery service.
    async fn repair_event_book(&self, event_book: EventBook) -> Result<EventBook, Status> {
        // Check if repair is needed
        if event_book_repair::is_complete(&event_book) {
            return Ok(event_book);
        }

        // No repairer configured - log warning and pass through
        let Some(ref repairer) = self.repairer else {
            warn!(
                "Received incomplete EventBook but no repairer configured, passing through as-is"
            );
            return Ok(event_book);
        };

        // Repair the EventBook
        let mut repairer = repairer.lock().await;
        repairer.repair(event_book).await.map_err(|e| {
            error!(error = %e, "Failed to repair EventBook");
            Status::internal(format!("Failed to repair EventBook: {}", e))
        })
    }
}

impl Default for ProjectorCoordinatorService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl ProjectorCoordinator for ProjectorCoordinatorService {
    /// Handle events synchronously, returning a projection.
    async fn handle_sync(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();

        // Repair EventBook if incomplete
        let event_book = self.repair_event_book(event_book).await?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let projectors = self.projectors.read().await;
            projectors
                .iter()
                .filter(|conn| conn.config.synchronous)
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        // Return the first successful projection
        if let Some((config, mut client)) = connections.into_iter().next() {
            let req = Request::new(event_book.clone());
            match client.handle_sync(req).await {
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
        let event_book = self.repair_event_book(event_book).await?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let projectors = self.projectors.read().await;
            projectors
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        for (config, mut client) in connections {
            let req = Request::new(event_book.clone());

            if config.synchronous {
                match client.handle_sync(req).await {
                    Ok(_) => info!(projector.name = %config.name, "Projection completed"),
                    Err(e) => error!(projector.name = %config.name, error = %e, "Projector failed"),
                }
            } else {
                match client.handle(req).await {
                    Ok(_) => info!(projector.name = %config.name, "Async projection queued"),
                    Err(e) => {
                        warn!(projector.name = %config.name, error = %e, "Failed to queue projection")
                    }
                }
            }
        }

        Ok(Response::new(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, Cover, EventPage, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    fn make_complete_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    fn make_incomplete_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(5)), // Missing events 0-4
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_new_creates_empty_coordinator() {
        let coordinator = ProjectorCoordinatorService::new();
        assert!(coordinator.projectors.read().await.is_empty());
        assert!(coordinator.repairer.is_none());
    }

    #[tokio::test]
    async fn test_default_creates_empty_coordinator() {
        let coordinator = ProjectorCoordinatorService::default();
        assert!(coordinator.projectors.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_handle_sync_with_no_projectors_returns_empty_projection() {
        let coordinator = ProjectorCoordinatorService::new();
        let event_book = make_event_book();

        let response = coordinator.handle_sync(Request::new(event_book)).await;

        assert!(response.is_ok());
        let projection = response.unwrap().into_inner();
        assert!(projection.projector.is_empty());
        assert_eq!(projection.sequence, 0);
    }

    #[tokio::test]
    async fn test_handle_with_no_projectors_succeeds() {
        let coordinator = ProjectorCoordinatorService::new();
        let event_book = make_event_book();

        let response = coordinator.handle(Request::new(event_book)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_add_projector_invalid_address() {
        let coordinator = ProjectorCoordinatorService::new();
        let config = ProjectorConfig {
            name: "test".to_string(),
            address: "".to_string(), // Invalid
            synchronous: false,
        };

        let result = coordinator.add_projector(config).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_repair_event_book_complete_passes_through() {
        let coordinator = ProjectorCoordinatorService::new();
        let event_book = make_complete_event_book();

        let result = coordinator.repair_event_book(event_book.clone()).await;

        assert!(result.is_ok());
        let repaired = result.unwrap();
        assert_eq!(repaired.pages.len(), event_book.pages.len());
    }

    #[tokio::test]
    async fn test_repair_event_book_incomplete_without_repairer_warns() {
        let coordinator = ProjectorCoordinatorService::new();
        let event_book = make_incomplete_event_book();

        // Without a repairer, incomplete books pass through with a warning
        let result = coordinator.repair_event_book(event_book.clone()).await;

        assert!(result.is_ok());
        let passed = result.unwrap();
        // Still incomplete since no repairer
        assert_eq!(passed.pages.len(), 1);
    }
}
