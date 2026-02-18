//! gRPC handler adapters for standalone mode.
//!
//! Bridges between handler traits and gRPC clients, enabling:
//! - In-process `AggregateHandler` to be used as `ClientLogic` (no TCP bridge)

use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
use tonic::Status;
use tracing::instrument;

use crate::orchestration::aggregate::ClientLogic;
use crate::proto::business_response::Result as BusinessResult;
use crate::proto::{BusinessResponse, ContextualCommand, EventBook, Notification};

use super::traits::{AggregateHandler, ProcessManagerHandler};

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
    #[instrument(name = "adapter.aggregate.invoke", skip_all)]
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

/// Adapts an in-process `ProcessManagerHandler` as `ClientLogic`.
///
/// PMs are aggregates — they receive compensation Notifications through
/// the standard command routing infrastructure. This adapter only handles
/// Notification commands (for compensation), not regular commands.
///
/// Flow:
/// 1. PM command rejected → saga_origin.triggering_aggregate = PM cover
/// 2. Notification command routes to PM domain
/// 3. CommandRouter invokes this adapter
/// 4. PM's handle_revocation() processes the Notification
/// 5. Compensation events returned to be persisted
pub struct ProcessManagerHandlerAdapter {
    handler: Arc<dyn ProcessManagerHandler>,
}

impl ProcessManagerHandlerAdapter {
    /// Wrap a process manager handler as a `ClientLogic` implementation.
    pub fn new(handler: Arc<dyn ProcessManagerHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl ClientLogic for ProcessManagerHandlerAdapter {
    #[instrument(name = "adapter.pm.invoke", skip_all)]
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        // PM only accepts Notification commands for compensation
        let command_book = cmd
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;
        let page = command_book
            .pages
            .first()
            .ok_or_else(|| Status::invalid_argument("Empty command pages"))?;
        let command_any = page
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command payload"))?;

        if !command_any.type_url.ends_with(NOTIFICATION_SUFFIX) {
            return Err(Status::invalid_argument(
                "PM only accepts Notification commands for compensation",
            ));
        }

        // Decode Notification
        let notification = Notification::decode(command_any.value.as_slice()).map_err(|e| {
            Status::invalid_argument(format!("Failed to decode Notification: {}", e))
        })?;

        // PM state comes from cmd.events (loaded by CommandRouter)
        let pm_state = cmd.events.as_ref();

        // Call PM's revocation handler
        let (pm_events, revocation_response) =
            self.handler.handle_revocation(&notification, pm_state);

        // Return compensation events or revocation response
        match pm_events {
            Some(events) if !events.pages.is_empty() => Ok(BusinessResponse {
                result: Some(BusinessResult::Events(events)),
            }),
            _ => Ok(BusinessResponse {
                result: Some(BusinessResult::Revocation(revocation_response)),
            }),
        }
    }

    async fn replay(&self, _events: &EventBook) -> Result<prost_types::Any, Status> {
        // PMs don't support replay through this adapter
        // PM state is rebuilt via the normal PM flow
        Err(Status::unimplemented(
            "PM replay not supported through command adapter",
        ))
    }
}
