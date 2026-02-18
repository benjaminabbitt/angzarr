//! gRPC handler adapters for standalone mode.
//!
//! Bridges between handler traits and gRPC clients, enabling:
//! - In-process `AggregateHandler` to be used as `ClientLogic` (no TCP bridge)

use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
use tonic::Status;

use crate::orchestration::aggregate::ClientLogic;
use crate::proto::business_response::Result as BusinessResult;
use crate::proto::{BusinessResponse, ContextualCommand, EventBook, Notification};

use super::traits::AggregateHandler;

pub use crate::orchestration::projector::GrpcProjectorHandler;

/// Type URL suffix for Notification.
const NOTIFICATION_SUFFIX: &str = "Notification";

/// Adapts an in-process `AggregateHandler` as `ClientLogic`.
///
/// Eliminates the TCP bridge: calls the handler directly and wraps the
/// result in a `BusinessResponse`. Used by the standalone `Runtime` to avoid
/// spawning gRPC servers for Rust aggregate handlers.
///
/// Detects `Notification` and routes to `handle_revocation()` for compensation.
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
        // Check for rejection notifications
        if let Some(ref command_book) = cmd.command {
            if let Some(page) = command_book.pages.first() {
                if let Some(ref command_any) = page.command {
                    if command_any.type_url.ends_with(NOTIFICATION_SUFFIX) {
                        let notification = Notification::decode(command_any.value.as_slice())
                            .map_err(|e| {
                                Status::invalid_argument(format!(
                                    "Failed to decode Notification: {}",
                                    e
                                ))
                            })?;
                        return Ok(self.handler.handle_revocation(&notification));
                    }
                }
            }
        }

        // Normal command handling
        let events = self.handler.handle(cmd).await?;
        Ok(BusinessResponse {
            result: Some(BusinessResult::Events(events)),
        })
    }

    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        self.handler.replay(events).await
    }
}
