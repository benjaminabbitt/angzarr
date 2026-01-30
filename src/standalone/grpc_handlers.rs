//! gRPC handler adapters for standalone mode.
//!
//! Bridges between handler traits and gRPC clients, enabling:
//! - In-process `AggregateHandler` to be used as `BusinessLogic` (no TCP bridge)
//! - Remote gRPC `ProjectorCoordinator` to be used as `ProjectorHandler`

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tonic::Status;

use crate::orchestration::aggregate::BusinessLogic;
use crate::proto::business_response::Result as BusinessResult;
use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use crate::proto::{
    BusinessResponse, ContextualCommand, EventBook, Projection, SyncEventBook, SyncMode,
};

use super::traits::{AggregateHandler, ProjectorHandler};

/// Adapts an in-process `AggregateHandler` as `BusinessLogic`.
///
/// Eliminates the TCP bridge: calls the handler directly and wraps the
/// result in a `BusinessResponse`. Used by the standalone `Runtime` to avoid
/// spawning gRPC servers for Rust aggregate handlers.
pub struct AggregateHandlerAdapter {
    handler: Arc<dyn AggregateHandler>,
}

impl AggregateHandlerAdapter {
    /// Wrap an aggregate handler as a `BusinessLogic` implementation.
    pub fn new(handler: Arc<dyn AggregateHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl BusinessLogic for AggregateHandlerAdapter {
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        let events = self.handler.handle(cmd).await?;
        Ok(BusinessResponse {
            result: Some(BusinessResult::Events(events)),
        })
    }
}

/// Wraps a gRPC `ProjectorCoordinator` client as a `ProjectorHandler`.
///
/// Forwards calls to a remote projector process via gRPC (TCP or UDS).
/// Used by the standalone binary to call polyglot projector processes.
pub struct GrpcProjectorHandler {
    client: Mutex<ProjectorCoordinatorClient<tonic::transport::Channel>>,
}

impl GrpcProjectorHandler {
    /// Wrap a gRPC projector client as a `ProjectorHandler`.
    pub fn new(client: ProjectorCoordinatorClient<tonic::transport::Channel>) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }
}

#[async_trait]
impl ProjectorHandler for GrpcProjectorHandler {
    async fn handle(&self, events: &EventBook, mode: super::traits::ProjectionMode) -> Result<Projection, Status> {
        // Skip remote gRPC call in speculative mode — cannot control remote side effects.
        if mode == super::traits::ProjectionMode::Speculate {
            return Ok(Projection::default());
        }
        let sync_book = SyncEventBook {
            events: Some(events.clone()),
            sync_mode: SyncMode::Simple.into(),
        };
        Ok(self
            .client
            .lock()
            .await
            .handle_sync(sync_book)
            .await?
            .into_inner())
    }
}
