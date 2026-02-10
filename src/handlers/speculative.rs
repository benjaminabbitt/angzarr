//! Speculative execution service handler.
//!
//! Provides gRPC endpoints for executing commands, projectors, sagas,
//! and process managers speculatively without persisting side effects.

use std::sync::Arc;

use tonic::{Request, Response, Status};
use tracing::{debug, warn};

use crate::discovery::ServiceDiscovery;
use crate::handlers::gateway::{errmsg, CommandRouter};
use crate::orchestration::correlation::extract_correlation_id;
use crate::proto::speculative_service_server::SpeculativeService;
use crate::proto::{
    CommandResponse, DryRunRequest, ProcessManagerHandleResponse, Projection, SagaResponse,
    SpeculatePmRequest, SpeculateProjectorRequest, SpeculateSagaRequest,
};
use crate::proto_ext::{correlated_request, CoverExt};

/// Speculative execution service.
///
/// Executes commands speculatively against temporal or provided state
/// without persisting. Projector speculation routes to deployed coordinators.
/// Saga/PM speculation requires standalone mode with direct access.
pub struct SpeculativeHandler {
    discovery: Arc<dyn ServiceDiscovery>,
    command_router: CommandRouter,
}

impl SpeculativeHandler {
    /// Create a new speculative handler with service discovery.
    pub fn new(discovery: Arc<dyn ServiceDiscovery>) -> Self {
        Self {
            discovery: discovery.clone(),
            command_router: CommandRouter::new(discovery),
        }
    }
}

#[tonic::async_trait]
impl SpeculativeService for SpeculativeHandler {
    /// Execute command against temporal state without persisting.
    #[tracing::instrument(name = "speculative.dry_run_command", skip_all)]
    async fn dry_run_command(
        &self,
        request: Request<DryRunRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let dry_run_request = request.into_inner();

        let correlation_id = match dry_run_request.command.as_ref() {
            Some(cmd) => extract_correlation_id(cmd)?,
            None => {
                return Err(Status::invalid_argument(
                    "DryRunRequest must have a command",
                ))
            }
        };

        debug!("Executing command (dry-run)");

        let response = self
            .command_router
            .forward_dry_run(dry_run_request, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }

    /// Execute projector against provided events without persisting.
    ///
    /// Routes to the projector coordinator's speculative handler.
    #[tracing::instrument(name = "speculative.projector", skip_all)]
    async fn speculate_projector(
        &self,
        request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        let req = request.into_inner();
        let projector_name = &req.projector_name;

        let events = req.events.ok_or_else(|| {
            Status::invalid_argument("SpeculateProjectorRequest must have events")
        })?;

        debug!(projector = %projector_name, "Routing speculative projector request");

        // Get projector coordinator by name
        let mut client = self
            .discovery
            .get_projector_by_name(projector_name)
            .await
            .map_err(|e| {
                // Log full error internally
                warn!(projector = %projector_name, error = %e, "Projector not found");
                // Return sanitized message to client
                Status::not_found(errmsg::PROJECTOR_NOT_FOUND)
            })?;

        // Call speculative handler
        let correlation_id = events.correlation_id().to_string();

        let response = client
            .handle_speculative(correlated_request(events, &correlation_id))
            .await
            .map_err(|e| {
                warn!(projector = %projector_name, error = %e, "Speculative projection failed");
                e
            })?;

        Ok(response)
    }

    /// Execute saga against provided events without persisting.
    ///
    /// Not implemented in gateway mode - requires standalone mode with direct access.
    #[tracing::instrument(name = "speculative.saga", skip_all)]
    async fn speculate_saga(
        &self,
        _request: Request<SpeculateSagaRequest>,
    ) -> Result<Response<SagaResponse>, Status> {
        Err(Status::unimplemented(
            "Saga speculation requires standalone mode. Use standalone client for speculative saga execution.",
        ))
    }

    /// Execute process manager against provided context without persisting.
    ///
    /// Not implemented in gateway mode - requires standalone mode with direct access.
    #[tracing::instrument(name = "speculative.process_manager", skip_all)]
    async fn speculate_process_manager(
        &self,
        _request: Request<SpeculatePmRequest>,
    ) -> Result<Response<ProcessManagerHandleResponse>, Status> {
        Err(Status::unimplemented(
            "Process manager speculation requires standalone mode. Use standalone client for speculative PM execution.",
        ))
    }
}
