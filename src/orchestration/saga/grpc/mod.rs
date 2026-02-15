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
use crate::config::SagaCompensationConfig;
use crate::proto::aggregate_coordinator_service_client::AggregateCoordinatorServiceClient;
use crate::proto::saga_service_client::SagaServiceClient;
use crate::proto::{
    CommandBook, Cover, Edition, EventBook, SagaExecuteRequest, SagaPrepareRequest,
};
use crate::proto_ext::EditionExt;
use crate::proto_ext::{correlated_request, CoverExt};
use crate::utils::saga_compensation::{build_notification_command_book, CompensationContext};

use super::{SagaContextFactory, SagaRetryContext};

/// gRPC saga context.
///
/// Saga prepare/execute calls go to a remote `SagaServiceClient` via gRPC.
/// Compensation for rejected commands uses a separate `AggregateCoordinatorServiceClient`.
/// Command execution and destination fetching are handled externally by the caller.
pub struct GrpcSagaContext {
    saga_client: Arc<Mutex<SagaServiceClient<tonic::transport::Channel>>>,
    publisher: Arc<dyn EventBus>,
    compensation_config: SagaCompensationConfig,
    compensation_handler:
        Option<Arc<Mutex<AggregateCoordinatorServiceClient<tonic::transport::Channel>>>>,
    source: EventBook,
}

impl GrpcSagaContext {
    /// Create a new gRPC saga context for one saga invocation.
    pub fn new(
        saga_client: Arc<Mutex<SagaServiceClient<tonic::transport::Channel>>>,
        publisher: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
        compensation_handler: Option<
            Arc<Mutex<AggregateCoordinatorServiceClient<tonic::transport::Channel>>>,
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
        let correlation_id = self.source.correlation_id();
        let edition = self.source.edition().to_string();
        let mut client = self.saga_client.lock().await;
        let request = SagaPrepareRequest {
            source: Some(self.source.clone()),
        };
        let response = client
            .prepare(correlated_request(request, correlation_id))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Stamp source edition onto outgoing covers
        let mut covers = response.into_inner().destinations;
        for cover in &mut covers {
            if cover.edition.as_ref().is_none_or(|e| e.is_empty()) {
                cover.edition = Some(Edition {
                    name: edition.clone(),
                    divergences: vec![],
                });
            }
        }
        Ok(covers)
    }

    async fn re_execute_saga(
        &self,
        destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        let correlation_id = self.source.correlation_id();
        let edition = self.source.edition().to_string();
        let mut client = self.saga_client.lock().await;
        let request = SagaExecuteRequest {
            source: Some(self.source.clone()),
            destinations,
        };
        let response = client
            .execute(correlated_request(request, correlation_id))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Stamp source edition onto outgoing command covers
        let commands = response
            .into_inner()
            .commands
            .into_iter()
            .map(|mut cmd| {
                if let Some(ref mut c) = cmd.cover {
                    if c.edition.as_ref().is_none_or(|e| e.is_empty()) {
                        c.edition = Some(Edition {
                            name: edition.clone(),
                            divergences: vec![],
                        });
                    }
                }
                cmd
            })
            .collect();
        Ok(commands)
    }

    fn source_cover(&self) -> Option<&Cover> {
        self.source.cover.as_ref()
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
/// sends a Notification with RejectionNotification payload to the
/// triggering aggregate for compensation.
/// If compensation fails or client logic requests it, emits a fallback event.
async fn handle_command_rejection(
    rejected_command: &CommandBook,
    rejection_error: &tonic::Status,
    handler: &mut AggregateCoordinatorServiceClient<tonic::transport::Channel>,
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

    let notification_command = match build_notification_command_book(&context) {
        Ok(cmd) => cmd,
        Err(e) => {
            error!(
                saga = %saga_name,
                error = %e,
                "Failed to build notification, emitting fallback event"
            );
            emit_fallback_event(
                &context,
                "Failed to build notification",
                publisher,
                config,
            )
            .await;
            return;
        }
    };

    let triggering_domain = notification_command.domain().to_string();
    let correlation_id = notification_command.correlation_id().to_string();

    info!(
        saga = %saga_name,
        triggering_domain = %triggering_domain,
        "Sending rejection Notification to triggering aggregate"
    );

    match handler
        .handle(correlated_request(notification_command, &correlation_id))
        .await
    {
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
                    "Rejection notification handled, no compensation events produced"
                );
            }
        }
        Err(e) => {
            error!(
                saga = %saga_name,
                triggering_domain = %triggering_domain,
                error = %e,
                "Rejection notification failed, emitting fallback event"
            );
            emit_fallback_event(
                &context,
                &format!("Rejection notification failed: {}", e),
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
    saga_client: Arc<Mutex<SagaServiceClient<tonic::transport::Channel>>>,
    publisher: Arc<dyn EventBus>,
    compensation_config: SagaCompensationConfig,
    compensation_handler:
        Option<Arc<Mutex<AggregateCoordinatorServiceClient<tonic::transport::Channel>>>>,
    name: String,
}

impl GrpcSagaContextFactory {
    /// Create a new factory with saga client and compensation configuration.
    pub fn new(
        saga_client: Arc<Mutex<SagaServiceClient<tonic::transport::Channel>>>,
        publisher: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
        compensation_handler: Option<
            Arc<Mutex<AggregateCoordinatorServiceClient<tonic::transport::Channel>>>,
        >,
        name: String,
    ) -> Self {
        Self {
            saga_client,
            publisher,
            compensation_config,
            compensation_handler,
            name,
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

    fn name(&self) -> &str {
        &self.name
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
