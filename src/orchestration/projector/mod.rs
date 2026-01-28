//! Projector orchestration abstraction.
//!
//! `ProjectorContext` defines the interface for projector event handling.
//! Implementations provide in-process (local) or gRPC (distributed) delegation.
//!
//! - `local/`: in-process ProjectorHandler calls
//! - `grpc/`: remote ProjectorCoordinatorClient calls

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use async_trait::async_trait;

use crate::proto::{EventBook, Projection};

/// Projector-specific operation abstracted over transport.
///
/// Implementations handle events via in-process handler (local) or
/// gRPC ProjectorCoordinator client (distributed).
#[async_trait]
pub trait ProjectorContext: Send + Sync {
    /// Handle events and produce a projection result.
    async fn handle_events(&self, events: &EventBook) -> Result<Projection, tonic::Status>;
}
