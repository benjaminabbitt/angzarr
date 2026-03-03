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
    projector_coordinator_service_server::ProjectorCoordinatorService,
    projector_service_client::ProjectorServiceClient, EventBook, EventRequest, Projection,
    SpeculateProjectorRequest,
};
use crate::proto_ext::{correlated_request, CoverExt};
use crate::services::event_book_repair::EventBookRepairer;

/// Connected projector client.
struct ProjectorConnection {
    config: ServiceEndpoint,
    client: ProjectorServiceClient<Channel>,
}

/// Projector coordinator service.
///
/// Distributes events to all registered projectors. Before forwarding,
/// checks if EventBooks are complete and fetches missing history from
/// the EventQuery service if needed.
pub struct ProjectorCoord {
    projectors: Arc<RwLock<Vec<ProjectorConnection>>>,
    repairer: Arc<Mutex<EventBookRepairer>>,
}

impl ProjectorCoord {
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
        let client = ProjectorServiceClient::new(channel);

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
impl ProjectorCoordinatorService for ProjectorCoord {
    /// Handle events synchronously, returning a projection.
    async fn handle_sync(
        &self,
        request: Request<EventRequest>,
    ) -> Result<Response<Projection>, Status> {
        let sync_request = request.into_inner();
        let event_book = sync_request
            .events
            .ok_or_else(|| Status::invalid_argument(super::errmsg::EVENT_REQUEST_MISSING_EVENTS))?;

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
        request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        let speculate_request = request.into_inner();
        let event_book = speculate_request.events.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::SPECULATE_PROJ_MISSING_EVENTS)
        })?;

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
#[path = "projector_coord.test.rs"]
mod tests;
