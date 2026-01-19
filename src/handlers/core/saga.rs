//! Saga event handler for saga sidecar.
//!
//! Receives events from the event bus and forwards them to saga
//! coordinator services. The coordinator ensures sagas receive complete
//! EventBooks by fetching missing history from the EventQuery service.
//!
//! When sagas produce commands, they are executed via the command handler.
//! When saga commands are rejected, compensation flow is initiated:
//! - RevokeEventCommand is sent to the triggering aggregate
//! - Business logic can provide compensation events or request framework action
//! - Fallback events are recorded for unhandled compensation failures
//!
//! All outputs preserve the original correlation_id for streaming.

use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::bus::{BusError, EventBus, EventHandler};
use crate::clients::SagaCompensationConfig;
use crate::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use crate::proto::saga_coordinator_client::SagaCoordinatorClient;
use crate::proto::{CommandBook, EventBook, SyncEventBook, SyncMode};
use crate::utils::saga_compensation::{build_revoke_command_book, CompensationContext};

/// Event handler that forwards events to a saga gRPC service.
///
/// Calls `handle_sync` to get saga-produced commands and event books.
/// Commands are executed via the command handler (which publishes resulting
/// events). When commands are rejected, compensation is attempted.
pub struct SagaEventHandler {
    client: Arc<Mutex<SagaCoordinatorClient<tonic::transport::Channel>>>,
    command_handler: Option<Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>>,
    publisher: Arc<dyn EventBus>,
    compensation_config: SagaCompensationConfig,
}

impl SagaEventHandler {
    /// Create a new saga event handler without command execution capability.
    ///
    /// Saga-produced commands will be logged but not executed.
    pub fn new(
        client: SagaCoordinatorClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: None,
            publisher,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create a new saga event handler with command execution capability.
    ///
    /// Saga-produced commands will be executed via the command handler.
    pub fn with_command_handler(
        client: SagaCoordinatorClient<tonic::transport::Channel>,
        command_handler: AggregateCoordinatorClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: Some(Arc::new(Mutex::new(command_handler))),
            publisher,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create with full configuration including compensation settings.
    pub fn with_config(
        client: SagaCoordinatorClient<tonic::transport::Channel>,
        command_handler: AggregateCoordinatorClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: Some(Arc::new(Mutex::new(command_handler))),
            publisher,
            compensation_config,
        }
    }
}

impl EventHandler for SagaEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let client = self.client.clone();
        let command_handler = self.command_handler.clone();
        let publisher = self.publisher.clone();
        let compensation_config = self.compensation_config.clone();

        Box::pin(async move {
            let book_owned = (*book).clone();
            let correlation_id = book_owned.correlation_id.clone();

            // Call saga coordinator handle_sync to get resulting commands
            // The coordinator will repair incomplete EventBooks if needed
            let mut client = client.lock().await;
            let sync_request = SyncEventBook {
                events: Some(book_owned),
                sync_mode: SyncMode::Simple.into(),
            };
            let response = client
                .handle_sync(sync_request)
                .await
                .map_err(BusError::Grpc)?;
            let result = response.into_inner();

            debug!(
                correlation_id = %correlation_id,
                commands = result.commands.len(),
                "Saga produced commands"
            );

            // Execute saga-produced commands via command handler
            if !result.commands.is_empty() {
                if let Some(ref cmd_handler) = command_handler {
                    let mut handler = cmd_handler.lock().await;

                    for mut command_book in result.commands {
                        // Ensure correlation_id is propagated
                        if command_book.correlation_id.is_empty() {
                            command_book.correlation_id = correlation_id.clone();
                        }

                        let domain = command_book
                            .cover
                            .as_ref()
                            .map(|c| c.domain.as_str())
                            .unwrap_or("unknown");

                        info!(
                            correlation_id = %command_book.correlation_id,
                            domain = %domain,
                            "Executing saga-produced command via command handler"
                        );

                        // Execute command - resulting events will be published to AMQP
                        // by the command handler, which will then stream back to client
                        match handler.handle(command_book.clone()).await {
                            Ok(response) => {
                                let sync_resp = response.into_inner();
                                let has_events = sync_resp.events.is_some();
                                debug!(
                                    correlation_id = %correlation_id,
                                    domain = %domain,
                                    has_events = has_events,
                                    projections = sync_resp.projections.len(),
                                    "Saga command executed successfully"
                                );
                            }
                            Err(e) => {
                                // Command was rejected - attempt compensation
                                handle_command_rejection(
                                    &command_book,
                                    &e,
                                    &mut handler,
                                    &publisher,
                                    &compensation_config,
                                )
                                .await;
                                // Continue processing other commands
                            }
                        }
                    }
                } else {
                    warn!(
                        correlation_id = %correlation_id,
                        command_count = result.commands.len(),
                        "Saga produced commands but no command handler configured - commands dropped"
                    );
                }
            }

            Ok(())
        })
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

    // Check if this is a saga command (has saga_origin)
    let Some(context) =
        CompensationContext::from_rejected_command(rejected_command, rejection_reason.clone())
    else {
        // Not a saga command - just log and return
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

    // Build RevokeEventCommand to send to triggering aggregate
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

    // Send RevokeEventCommand via command handler
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
                // Business logic handled revocation but produced no events
                // This is acceptable - business may have decided no compensation needed
                debug!(
                    saga = %saga_name,
                    triggering_domain = %triggering_domain,
                    "Revocation handled, no compensation events produced"
                );
            }
        }
        Err(e) => {
            // RevokeEventCommand also failed - emit fallback event
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
