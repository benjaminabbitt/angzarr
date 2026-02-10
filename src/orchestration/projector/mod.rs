//! Projector abstraction shared across standalone and distributed modes.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tonic::Status;

use crate::proto::projector_client::ProjectorClient;
use crate::proto::{ComponentDescriptor, EventBook, Projection};
use crate::proto_ext::{correlated_request, CoverExt};

/// Execution mode for projectors.
///
/// Passed to `ProjectorHandler::handle()` so implementations can skip
/// persistence during speculative execution while keeping all business
/// logic (event decoding, field computation) identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    /// Normal execution: compute and persist projection.
    Execute,
    /// Speculative execution: compute projection, skip persistence.
    ///
    /// The handler must produce the same `Projection` as `Execute` mode
    /// but must NOT write to databases, files, or external systems.
    Speculate,
}

/// Projector handler for building read models.
///
/// Implement this trait to react to events and update read models.
/// Projectors can be synchronous (blocking command response) or
/// asynchronous (running in background).
///
/// The same handler instance is used for both normal and speculative
/// execution. Business logic runs identically in both modes — only
/// persistence side effects are gated on `ProjectionMode`.
#[async_trait]
pub trait ProjectorHandler: Send + Sync + 'static {
    /// Self-description: component type, subscribed domains, handled event types.
    fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor::default()
    }

    /// Handle events and update read model.
    ///
    /// `mode` controls whether persistence side effects should occur:
    /// - `Execute`: compute and persist (normal path)
    /// - `Speculate`: compute only, skip all writes
    ///
    /// Returns a Projection with any data to include in command response
    /// (only used for synchronous projectors).
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status>;
}

/// gRPC projector handler that forwards to a remote `Projector` service.
///
/// Client logic implements the simple `Projector` service (not `ProjectorCoordinator`).
/// Skips calls in `Speculate` mode since remote side effects can't be controlled.
pub struct GrpcProjectorHandler {
    client: Arc<Mutex<ProjectorClient<tonic::transport::Channel>>>,
}

impl GrpcProjectorHandler {
    /// Wrap a gRPC projector client as a `ProjectorHandler`.
    pub fn new(client: ProjectorClient<tonic::transport::Channel>) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }
}

#[async_trait]
impl ProjectorHandler for GrpcProjectorHandler {
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status> {
        if mode == ProjectionMode::Speculate {
            return Ok(Projection::default());
        }
        let correlation_id = events.correlation_id();
        Ok(self
            .client
            .lock()
            .await
            .handle(correlated_request(events.clone(), correlation_id))
            .await?
            .into_inner())
    }
}
