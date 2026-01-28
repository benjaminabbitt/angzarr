//! gRPC projector context.
//!
//! Delegates event handling to remote `ProjectorCoordinatorClient` via gRPC.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use crate::proto::{EventBook, Projection, SyncEventBook, SyncMode};

use super::ProjectorContext;

/// gRPC projector context that calls remote ProjectorCoordinator service.
pub struct GrpcProjectorContext {
    client: Arc<Mutex<ProjectorCoordinatorClient<tonic::transport::Channel>>>,
}

impl GrpcProjectorContext {
    /// Create with a gRPC ProjectorCoordinator client.
    pub fn new(client: Arc<Mutex<ProjectorCoordinatorClient<tonic::transport::Channel>>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ProjectorContext for GrpcProjectorContext {
    async fn handle_events(&self, events: &EventBook) -> Result<Projection, tonic::Status> {
        let mut client = self.client.lock().await;
        let sync_request = SyncEventBook {
            events: Some(events.clone()),
            sync_mode: SyncMode::Simple.into(),
        };
        let response = client.handle_sync(sync_request).await?;
        Ok(response.into_inner())
    }
}
