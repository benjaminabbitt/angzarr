//! Command gateway handler for angzarr-gateway service.
//!
//! The gateway service receives commands, forwards them to business coordinators
//! based on domain routing, and optionally streams back resulting events.

mod command_router;
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
use crate::proto::command_gateway_server::CommandGateway;
use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{CommandBook, CommandResponse, DryRunRequest, EventBook, SyncCommandBook};

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
    async fn execute(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let mut command_book = request.into_inner();
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (unary)");

        let response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }

    /// Sync execute - waits for projectors/sagas based on sync_mode.
    async fn execute_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_request = request.into_inner();
        let mut command_book = sync_request
            .command
            .ok_or_else(|| Status::invalid_argument("SyncCommandBook must have a command"))?;
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (sync)");

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
    async fn execute_stream(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let stream_handler = self.stream_handler.as_ref().ok_or_else(|| {
            Status::unimplemented("Event streaming not available (embedded mode)")
        })?;

        let mut command_book = request.into_inner();
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (stream)");

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

    /// Dry-run execute — execute command against temporal state without persisting.
    async fn dry_run_execute(
        &self,
        request: Request<DryRunRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let mut dry_run_request = request.into_inner();

        // Ensure correlation ID on the embedded command
        let correlation_id = match dry_run_request.command.as_mut() {
            Some(cmd) => CommandRouter::ensure_correlation_id(cmd)?,
            None => {
                return Err(Status::invalid_argument(
                    "DryRunRequest must have a command",
                ))
            }
        };

        debug!(correlation_id = %correlation_id, "Executing command (dry-run)");

        let response = self
            .command_router
            .forward_dry_run(dry_run_request, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests;
