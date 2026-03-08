//! gRPC client logic implementation.
//!
//! Provides `GrpcBusinessLogic` which wraps a tonic gRPC client for invoking
//! aggregate business logic over TCP, UDS, or duplex channels.

use async_trait::async_trait;
use tokio::sync::Mutex;
use tonic::Status;

use crate::proto::{
    command_handler_service_client::CommandHandlerServiceClient, BusinessResponse,
    ContextualCommand, EventBook, ReplayRequest,
};

use super::traits::ClientLogic;
use super::types::FactContext;

/// client logic invocation via gRPC `AggregateClient`.
///
/// Wraps a tonic `AggregateClient` channel (TCP, UDS, or duplex).
pub struct GrpcBusinessLogic {
    client: Mutex<CommandHandlerServiceClient<tonic::transport::Channel>>,
}

impl GrpcBusinessLogic {
    /// Wrap a gRPC aggregate client as a `ClientLogic` implementation.
    pub fn new(client: CommandHandlerServiceClient<tonic::transport::Channel>) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }
}

#[async_trait]
impl ClientLogic for GrpcBusinessLogic {
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        Ok(self.client.lock().await.handle(cmd).await?.into_inner())
    }

    async fn invoke_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
        // Default: pass through facts unchanged
        Ok(ctx.facts)
    }

    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        let request = ReplayRequest {
            events: events.pages.clone(),
            base_snapshot: events.snapshot.clone(),
        };
        let response = self.client.lock().await.replay(request).await?.into_inner();
        response
            .state
            .ok_or_else(|| Status::internal(crate::orchestration::errmsg::REPLAY_MISSING_STATE))
    }
}
