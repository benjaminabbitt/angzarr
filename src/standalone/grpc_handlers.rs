//! gRPC handler adapters for standalone mode.
//!
//! Bridges between handler traits and gRPC clients, enabling:
//! - In-process `AggregateHandler` to be used as `ClientLogic` (no TCP bridge)

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;

use crate::orchestration::aggregate::ClientLogic;
use crate::proto::business_response::Result as BusinessResult;
use crate::proto::{BusinessResponse, ContextualCommand};

use super::traits::AggregateHandler;

pub use crate::orchestration::projector::GrpcProjectorHandler;

/// Adapts an in-process `AggregateHandler` as `ClientLogic`.
///
/// Eliminates the TCP bridge: calls the handler directly and wraps the
/// result in a `BusinessResponse`. Used by the standalone `Runtime` to avoid
/// spawning gRPC servers for Rust aggregate handlers.
pub struct AggregateHandlerAdapter {
    handler: Arc<dyn AggregateHandler>,
}

impl AggregateHandlerAdapter {
    /// Wrap an aggregate handler as a `ClientLogic` implementation.
    pub fn new(handler: Arc<dyn AggregateHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl ClientLogic for AggregateHandlerAdapter {
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        let events = self.handler.handle(cmd).await?;
        Ok(BusinessResponse {
            result: Some(BusinessResult::Events(events)),
        })
    }
}
