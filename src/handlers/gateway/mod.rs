//! Command gateway handler for angzarr-gateway service.
//!
//! The gateway service receives commands, forwards them to business coordinators
//! based on domain routing, and optionally streams back resulting events.

mod command_router;
pub mod errmsg;
mod query_proxy;
mod stream_handler;

pub use command_router::{map_discovery_error, CommandRouter};
pub use query_proxy::EventQueryProxy;
pub use stream_handler::StreamHandler;

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::discovery::ServiceDiscovery;
use crate::orchestration::correlation::extract_correlation_id;
use crate::proto::command_gateway_server::CommandGateway;
use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{CommandBook, CommandResponse, EventBook, SyncCommandBook};

/// Command gateway service.
///
/// Receives commands, forwards to business coordinator based on domain routing,
/// and optionally streams back resulting events from the event stream service.
pub struct GatewayService {
    command_router: CommandRouter,
    stream_handler: Option<StreamHandler>,
}

impl GatewayService {
    /// Create a new gateway service with service discovery for domain routing.
    ///
    /// `stream_client` is optional - when `None`, streaming is disabled (embedded mode).
    pub fn new(
        discovery: Arc<dyn ServiceDiscovery>,
        stream_client: Option<EventStreamClient<tonic::transport::Channel>>,
        default_stream_timeout: Duration,
    ) -> Self {
        Self {
            command_router: CommandRouter::new(discovery),
            stream_handler: stream_client.map(|c| StreamHandler::new(c, default_stream_timeout)),
        }
    }
}

#[tonic::async_trait]
impl CommandGateway for GatewayService {
    /// Unary execute - returns immediate response only, no streaming.
    #[tracing::instrument(name = "gateway.execute", skip_all)]
    async fn execute(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command_book = request.into_inner();
        let correlation_id = extract_correlation_id(&command_book)?;

        debug!("Executing command (unary)");

        let response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }

    /// Sync execute - waits for projectors/sagas based on sync_mode.
    #[tracing::instrument(name = "gateway.execute_sync", skip_all)]
    async fn execute_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_request = request.into_inner();
        let command_book = sync_request
            .command
            .ok_or_else(|| Status::invalid_argument("SyncCommandBook must have a command"))?;
        let correlation_id = extract_correlation_id(&command_book)?;

        debug!("Executing command (sync)");

        let response = self
            .command_router
            .forward_command_sync(command_book, sync_request.sync_mode, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }

    type ExecuteStreamStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Streaming execute - streams events until client disconnects.
    ///
    /// Returns `Unimplemented` if streaming is disabled (embedded mode).
    #[tracing::instrument(name = "gateway.execute_stream", skip_all)]
    async fn execute_stream(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let stream_handler = self.stream_handler.as_ref().ok_or_else(|| {
            Status::unimplemented("Event streaming not available (embedded mode)")
        })?;

        let command_book = request.into_inner();
        let correlation_id = extract_correlation_id(&command_book)?;

        debug!("Executing command (stream)");

        // Subscribe BEFORE sending command
        let event_stream = stream_handler.subscribe(&correlation_id).await?;

        // Forward command
        let sync_response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;

        // Create stream with default timeout, no count limit
        let stream = stream_handler.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            None,
            stream_handler.default_timeout(),
        );

        Ok(Response::new(stream))
    }
}

#[cfg(test)]
mod tests;
