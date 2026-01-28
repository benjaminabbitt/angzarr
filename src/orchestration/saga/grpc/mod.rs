//! gRPC saga context.
//!
//! Implements `SagaRetryContext` via gRPC clients for command execution,
//! event fetching, and saga invocation. Includes compensation flow for
//! rejected commands.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::bus::EventBus;
use crate::clients::SagaCompensationConfig;
use crate::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use crate::proto::saga_client::SagaClient;
use crate::proto::{CommandBook, Cover, EventBook, SagaExecuteRequest, SagaPrepareRequest};
use crate::utils::saga_compensation::{build_revoke_command_book, CompensationContext};

use super::{SagaContextFactory, SagaRetryContext};

/// gRPC saga context.
///
/// Saga prepare/execute calls go to a remote `SagaClient` via gRPC.
/// Compensation for rejected commands uses a separate `AggregateCoordinatorClient`.
/// Command execution and destination fetching are handled externally by the caller.
pub struct GrpcSagaContext {
    saga_client: Arc<Mutex<SagaClient<tonic::transport::Channel>>>,
    publisher: Arc<dyn EventBus>,
    compensation_config: SagaCompensationConfig,
    compensation_handler: Option<Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>>,
    source: EventBook,
}

impl GrpcSagaContext {
    /// Create a new gRPC saga context for one saga invocation.
    pub fn new(
        saga_client: Arc<Mutex<SagaClient<tonic::transport::Channel>>>,
        publisher: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
        compensation_handler: Option<
            Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>,
        >,
        source: EventBook,
    ) -> Self {
        Self {
            saga_client,
            publisher,
            compensation_config,
            compensation_handler,
            source,
        }
    }
}

#[async_trait]
impl SagaRetryContext for GrpcSagaContext {
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        let mut client = self.saga_client.lock().await;
        let request = SagaPrepareRequest {
            source: Some(self.source.clone()),
        };
        let response = client
            .prepare(request)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(response.into_inner().destinations)
    }

    async fn re_execute_saga(
        &self,
        destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        let mut client = self.saga_client.lock().await;
        let request = SagaExecuteRequest {
            source: Some(self.source.clone()),
            destinations,
        };
        let response = client
            .execute(request)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(response.into_inner().commands)
    }

    async fn on_command_rejected(&self, command: &CommandBook, reason: &str) {
        if let Some(ref handler) = self.compensation_handler {
            let rejection_error = tonic::Status::internal(reason);
            let mut handler = handler.lock().await;
            handle_command_rejection(
                command,
                &rejection_error,
                &mut handler,
                &self.publisher,
                &self.compensation_config,
            )
            .await;
        } else {
            error!(reason = %reason, "Saga command rejected (no compensation path)");
        }
    }
}

/// Handle a rejected saga command by initiating compensation flow.
///
/// If the command has a saga_origin (meaning it came from a saga),
/// sends a RevokeEventCommand to the triggering aggregate for compensation.
/// If compensation fails or business logic requests it, emits a fallback event.
async fn handle_command_rejection(
    rejected_command: &CommandBook,
    rejection_error: &tonic::Status,
    handler: &mut AggregateCoordinatorClient<tonic::transport::Channel>,
    publisher: &Arc<dyn EventBus>,
    config: &SagaCompensationConfig,
) {
    let rejection_reason = rejection_error.message().to_string();

    let Some(context) =
        CompensationContext::from_rejected_command(rejected_command, rejection_reason.clone())
    else {
        error!(
            error = %rejection_error,
            "Command rejected (not a saga command, no compensation)"
        );
        return;
    };

    let saga_name = &context.saga_origin.saga_name;
    let domain = rejected_command
        .cover
        .as_ref()
        .map(|c| c.domain.as_str())
        .unwrap_or("unknown");

    warn!(
        saga = %saga_name,
        domain = %domain,
        reason = %rejection_reason,
        "Saga command rejected, initiating compensation"
    );

    let revoke_command = match build_revoke_command_book(&context) {
        Ok(cmd) => cmd,
        Err(e) => {
            error!(
                saga = %saga_name,
                error = %e,
                "Failed to build revoke command, emitting fallback event"
            );
            emit_fallback_event(
                &context,
                "Failed to build revoke command",
                publisher,
                config,
            )
            .await;
            return;
        }
    };

    let triggering_domain = revoke_command
        .cover
        .as_ref()
        .map(|c| c.domain.clone())
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        saga = %saga_name,
        triggering_domain = %triggering_domain,
        "Sending RevokeEventCommand to triggering aggregate"
    );

    match handler.handle(revoke_command).await {
        Ok(response) => {
            let sync_resp = response.into_inner();
            if sync_resp.events.is_some() {
                info!(
                    saga = %saga_name,
                    triggering_domain = %triggering_domain,
                    "Compensation events recorded successfully"
                );
            } else {
                debug!(
                    saga = %saga_name,
                    triggering_domain = %triggering_domain,
                    "Revocation handled, no compensation events produced"
                );
            }
        }
        Err(e) => {
            error!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                error = %e,
                "RevokeEventCommand failed, emitting fallback event"
            );
            emit_fallback_event(
                &context,
                &format!("RevokeEventCommand failed: {}", e),
                publisher,
                config,
            )
            .await;
        }
    }
}

/// Factory that produces `GrpcSagaContext` instances for distributed mode.
///
/// Captures long-lived gRPC clients for saga invocation and compensation.
/// Each call to `create()` produces a context for one saga invocation.
/// Command execution and destination fetching are handled by the event handler.
pub struct GrpcSagaContextFactory {
    saga_client: Arc<Mutex<SagaClient<tonic::transport::Channel>>>,
    publisher: Arc<dyn EventBus>,
    compensation_config: SagaCompensationConfig,
    compensation_handler: Option<Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>>,
}

impl GrpcSagaContextFactory {
    /// Create a new factory with saga client and compensation configuration.
    pub fn new(
        saga_client: Arc<Mutex<SagaClient<tonic::transport::Channel>>>,
        publisher: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
        compensation_handler: Option<
            Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>,
        >,
    ) -> Self {
        Self {
            saga_client,
            publisher,
            compensation_config,
            compensation_handler,
        }
    }
}

impl SagaContextFactory for GrpcSagaContextFactory {
    fn create(&self, source: Arc<EventBook>) -> Box<dyn SagaRetryContext> {
        Box::new(GrpcSagaContext::new(
            self.saga_client.clone(),
            self.publisher.clone(),
            self.compensation_config.clone(),
            self.compensation_handler.clone(),
            (*source).clone(),
        ))
    }
}

/// Emit a SagaCompensationFailed event to the fallback domain.
async fn emit_fallback_event(
    context: &CompensationContext,
    reason: &str,
    publisher: &Arc<dyn EventBus>,
    config: &SagaCompensationConfig,
) {
    use crate::utils::saga_compensation::build_compensation_failed_event_book;

    let event_book = build_compensation_failed_event_book(context, reason, config);

    info!(
        saga = %context.saga_origin.saga_name,
        domain = %config.fallback_domain,
        reason = %reason,
        "Emitting SagaCompensationFailed event"
    );

    if let Err(e) = publisher.publish(Arc::new(event_book)).await {
        error!(
            saga = %context.saga_origin.saga_name,
            error = %e,
            "Failed to publish SagaCompensationFailed event"
        );
    }
}
