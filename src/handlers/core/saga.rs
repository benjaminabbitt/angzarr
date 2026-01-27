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

use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::bus::{BusError, EventBus, EventHandler};
use crate::clients::SagaCompensationConfig;
use crate::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use crate::proto::event_query_client::EventQueryClient;
use crate::proto::saga_client::SagaClient;
use crate::proto::{CommandBook, Cover, EventBook, Query, SagaExecuteRequest, SagaPrepareRequest};
use crate::utils::retry::{is_retryable_status, RetryConfig};
use crate::utils::saga_compensation::{build_revoke_command_book, CompensationContext};

/// Event query router for fetching EventBooks from multiple domains.
///
/// Routes queries to appropriate EventQuery services based on domain.
#[derive(Clone)]
pub struct EventQueryRouter {
    clients: Arc<HashMap<String, Arc<Mutex<EventQueryClient<tonic::transport::Channel>>>>>,
}

impl EventQueryRouter {
    /// Create a new event query router with domain -> client mapping.
    pub fn new(clients: HashMap<String, EventQueryClient<tonic::transport::Channel>>) -> Self {
        let wrapped = clients
            .into_iter()
            .map(|(k, v)| (k, Arc::new(Mutex::new(v))))
            .collect();
        Self {
            clients: Arc::new(wrapped),
        }
    }

    /// Fetch an EventBook for the given cover.
    pub async fn fetch(&self, cover: &Cover) -> Result<EventBook, tonic::Status> {
        let domain = &cover.domain;
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| tonic::Status::invalid_argument("Cover must have root UUID"))?;

        let client = self.clients.get(domain).ok_or_else(|| {
            tonic::Status::not_found(format!("No EventQuery registered for domain: {}", domain))
        })?;

        let query = Query {
            cover: Some(Cover {
                domain: domain.clone(),
                root: Some(root.clone()),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let mut client = client.lock().await;
        let event_book = client.get_event_book(query).await?.into_inner();

        Ok(event_book)
    }
}

/// Command router for multi-domain command execution.
///
/// Routes commands to appropriate aggregate coordinators based on domain.
#[derive(Clone)]
pub struct CommandRouter {
    clients: Arc<HashMap<String, Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>>>,
}

impl CommandRouter {
    /// Create a new command router with domain -> client mapping.
    pub fn new(
        clients: HashMap<String, AggregateCoordinatorClient<tonic::transport::Channel>>,
    ) -> Self {
        let wrapped = clients
            .into_iter()
            .map(|(k, v)| (k, Arc::new(Mutex::new(v))))
            .collect();
        Self {
            clients: Arc::new(wrapped),
        }
    }

    /// Get client for a domain.
    pub async fn get_client(
        &self,
        domain: &str,
    ) -> Option<Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>> {
        self.clients.get(domain).cloned()
    }

    /// Execute a command by routing to the appropriate aggregate.
    pub async fn execute(
        &self,
        command_book: CommandBook,
    ) -> Result<crate::proto::CommandResponse, tonic::Status> {
        let domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let client = self.clients.get(domain).ok_or_else(|| {
            tonic::Status::not_found(format!("No aggregate registered for domain: {}", domain))
        })?;

        let mut client = client.lock().await;
        client
            .handle(command_book)
            .await
            .map(|r| r.into_inner())
    }
}

/// Event handler that forwards events to a saga gRPC service.
///
/// Implements two-phase saga protocol:
/// 1. Prepare: Ask saga which destination aggregates it needs
/// 2. Execute: Fetch destinations, call saga with source + destination state
///
/// Commands are executed via the command handler (which publishes resulting
/// events). When commands are rejected, compensation is attempted.
pub struct SagaEventHandler {
    client: Arc<Mutex<SagaClient<tonic::transport::Channel>>>,
    command_handler: Option<Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>>,
    command_router: Option<CommandRouter>,
    event_query_router: Option<EventQueryRouter>,
    publisher: Arc<dyn EventBus>,
    compensation_config: SagaCompensationConfig,
}

impl SagaEventHandler {
    /// Create a new saga event handler without command execution capability.
    ///
    /// Saga-produced commands will be logged but not executed.
    /// Two-phase protocol not supported (no event query router).
    pub fn new(
        client: SagaClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: None,
            command_router: None,
            event_query_router: None,
            publisher,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create a new saga event handler with command execution capability.
    ///
    /// Saga-produced commands will be executed via the command handler.
    /// Two-phase protocol not supported (no event query router).
    pub fn with_command_handler(
        client: SagaClient<tonic::transport::Channel>,
        command_handler: AggregateCoordinatorClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: Some(Arc::new(Mutex::new(command_handler))),
            command_router: None,
            event_query_router: None,
            publisher,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create a new saga event handler with multi-domain routing.
    ///
    /// Supports two-phase saga protocol:
    /// - event_query_router: Fetches destination EventBooks
    /// - command_router: Executes saga-produced commands
    pub fn with_routers(
        client: SagaClient<tonic::transport::Channel>,
        command_router: CommandRouter,
        event_query_router: EventQueryRouter,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: None,
            command_router: Some(command_router),
            event_query_router: Some(event_query_router),
            publisher,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create a new saga event handler with multi-domain command routing only.
    ///
    /// Saga-produced commands will be routed to appropriate aggregates by domain.
    /// Two-phase protocol not supported (no event query router).
    pub fn with_command_router(
        client: SagaClient<tonic::transport::Channel>,
        command_router: CommandRouter,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: None,
            command_router: Some(command_router),
            event_query_router: None,
            publisher,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create with full configuration including compensation settings.
    pub fn with_config(
        client: SagaClient<tonic::transport::Channel>,
        command_handler: AggregateCoordinatorClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_handler: Some(Arc::new(Mutex::new(command_handler))),
            command_router: None,
            event_query_router: None,
            publisher,
            compensation_config,
        }
    }
}

impl EventHandler for SagaEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let client = self.client.clone();
        let command_handler = self.command_handler.clone();
        let command_router = self.command_router.clone();
        let event_query_router = self.event_query_router.clone();
        let publisher = self.publisher.clone();
        let compensation_config = self.compensation_config.clone();

        Box::pin(async move {
            let book_owned = (*book).clone();
            let correlation_id = book_owned
                .cover
                .as_ref()
                .map(|c| c.correlation_id.clone())
                .unwrap_or_default();

            // Phase 1: Call prepare to get destination covers
            let mut client = client.lock().await;
            let prepare_request = SagaPrepareRequest {
                source: Some(book_owned.clone()),
            };
            let prepare_response = client
                .prepare(prepare_request)
                .await
                .map_err(BusError::Grpc)?;
            let destination_covers = prepare_response.into_inner().destinations;

            debug!(
                correlation_id = %correlation_id,
                destinations = destination_covers.len(),
                "Saga prepare returned destinations"
            );

            // Phase 2: Fetch destination EventBooks if any
            let destinations = if !destination_covers.is_empty() {
                if let Some(ref router) = event_query_router {
                    let mut fetched = Vec::with_capacity(destination_covers.len());
                    for cover in &destination_covers {
                        match router.fetch(cover).await {
                            Ok(event_book) => fetched.push(event_book),
                            Err(e) => {
                                warn!(
                                    correlation_id = %correlation_id,
                                    domain = %cover.domain,
                                    error = %e,
                                    "Failed to fetch destination EventBook, skipping"
                                );
                            }
                        }
                    }
                    fetched
                } else {
                    warn!(
                        correlation_id = %correlation_id,
                        destinations = destination_covers.len(),
                        "Saga needs destinations but no event_query_router configured"
                    );
                    vec![]
                }
            } else {
                vec![]
            };

            // Phase 3: Call execute with source and destinations
            let saga_request = SagaExecuteRequest {
                source: Some(book_owned.clone()),
                destinations,
            };
            let response = client.execute(saga_request).await.map_err(BusError::Grpc)?;
            let result = response.into_inner();

            debug!(
                correlation_id = %correlation_id,
                commands = result.commands.len(),
                "Saga produced commands"
            );

            // Execute saga-produced commands with retry on sequence conflict
            // On conflict: refresh destination state, call saga Execute again, retry
            let retry_config = RetryConfig::for_saga_commands();
            let mut commands = result.commands;
            let mut attempt = 0u32;

            'retry: while !commands.is_empty() && attempt <= retry_config.max_retries {
                let mut failed_commands = Vec::new();
                let mut needs_retry = false;

                // Try command router first (multi-domain), then single handler
                if let Some(ref router) = command_router {
                    for mut command_book in commands {
                        // Ensure correlation_id is set on cover
                        if let Some(ref mut cover) = command_book.cover {
                            if cover.correlation_id.is_empty() {
                                cover.correlation_id = correlation_id.clone();
                            }
                        }

                        let (domain, cmd_correlation_id) = command_book
                            .cover
                            .as_ref()
                            .map(|c| (c.domain.as_str(), c.correlation_id.as_str()))
                            .unwrap_or(("unknown", ""));

                        debug!(
                            correlation_id = %cmd_correlation_id,
                            domain = %domain,
                            attempt = attempt,
                            "Executing saga command via router"
                        );

                        match router.execute(command_book.clone()).await {
                            Ok(sync_resp) => {
                                debug!(
                                    correlation_id = %correlation_id,
                                    domain = %domain,
                                    has_events = sync_resp.events.is_some(),
                                    "Saga command executed successfully"
                                );
                            }
                            Err(e) if is_retryable_status(&e) => {
                                warn!(
                                    correlation_id = %correlation_id,
                                    domain = %domain,
                                    attempt = attempt,
                                    error = %e,
                                    "Sequence conflict, will retry with fresh state"
                                );
                                failed_commands.push(command_book);
                                needs_retry = true;
                            }
                            Err(e) => {
                                error!(
                                    correlation_id = %correlation_id,
                                    domain = %domain,
                                    error = %e,
                                    "Saga command rejected (non-retryable)"
                                );
                            }
                        }
                    }
                } else if let Some(ref cmd_handler) = command_handler {
                    let mut handler = cmd_handler.lock().await;

                    for mut command_book in commands {
                        // Ensure correlation_id is set on cover
                        if let Some(ref mut cover) = command_book.cover {
                            if cover.correlation_id.is_empty() {
                                cover.correlation_id = correlation_id.clone();
                            }
                        }

                        let (domain, cmd_correlation_id) = command_book
                            .cover
                            .as_ref()
                            .map(|c| (c.domain.as_str(), c.correlation_id.as_str()))
                            .unwrap_or(("unknown", ""));

                        debug!(
                            correlation_id = %cmd_correlation_id,
                            domain = %domain,
                            attempt = attempt,
                            "Executing saga command via handler"
                        );

                        match handler.handle(command_book.clone()).await {
                            Ok(response) => {
                                let sync_resp = response.into_inner();
                                debug!(
                                    correlation_id = %correlation_id,
                                    domain = %domain,
                                    has_events = sync_resp.events.is_some(),
                                    "Saga command executed successfully"
                                );
                            }
                            Err(e) if is_retryable_status(&e) => {
                                warn!(
                                    correlation_id = %correlation_id,
                                    domain = %domain,
                                    attempt = attempt,
                                    error = %e,
                                    "Sequence conflict, will retry with fresh state"
                                );
                                failed_commands.push(command_book);
                                needs_retry = true;
                            }
                            Err(e) => {
                                // Non-retryable error - attempt compensation
                                handle_command_rejection(
                                    &command_book,
                                    &e,
                                    &mut handler,
                                    &publisher,
                                    &compensation_config,
                                )
                                .await;
                            }
                        }
                    }
                } else {
                    warn!(
                        correlation_id = %correlation_id,
                        command_count = commands.len(),
                        "Saga produced commands but no command handler configured - commands dropped"
                    );
                    break 'retry;
                }

                // If no retries needed, we're done
                if !needs_retry {
                    break 'retry;
                }

                // Retry: call Prepare() again to get ALL destinations, fetch fresh state
                // This ensures saga gets complete state including domains where commands succeeded
                if let Some(ref eq_router) = event_query_router {
                    // Call Prepare() again to get full list of destination covers
                    let prepare_request = SagaPrepareRequest {
                        source: Some(book_owned.clone()),
                    };
                    let destination_covers = match client.prepare(prepare_request).await {
                        Ok(resp) => resp.into_inner().destinations,
                        Err(e) => {
                            warn!(
                                correlation_id = %correlation_id,
                                error = %e,
                                "Saga Prepare failed on retry, using failed commands only"
                            );
                            // Fallback: use failed commands' covers
                            failed_commands
                                .iter()
                                .filter_map(|cmd| cmd.cover.clone())
                                .collect()
                        }
                    };

                    // Fetch fresh state for ALL destinations (not just failed commands)
                    let mut fresh_destinations = Vec::new();
                    for cover in &destination_covers {
                        match eq_router.fetch(cover).await {
                            Ok(event_book) => fresh_destinations.push(event_book),
                            Err(e) => {
                                warn!(
                                    correlation_id = %correlation_id,
                                    domain = %cover.domain,
                                    error = %e,
                                    "Failed to fetch fresh state for retry"
                                );
                            }
                        }
                    }

                    info!(
                        correlation_id = %correlation_id,
                        attempt = attempt,
                        destinations = fresh_destinations.len(),
                        "Retry fetched fresh destination state"
                    );

                    // Call saga Execute again with fresh state for ALL destinations
                    let retry_request = SagaExecuteRequest {
                        source: Some(book_owned.clone()),
                        destinations: fresh_destinations,
                    };
                    match client.execute(retry_request).await {
                        Ok(response) => {
                            let retry_result = response.into_inner();
                            commands = retry_result.commands;
                            debug!(
                                correlation_id = %correlation_id,
                                attempt = attempt,
                                new_commands = commands.len(),
                                "Saga retry produced new commands"
                            );
                        }
                        Err(e) => {
                            error!(
                                correlation_id = %correlation_id,
                                attempt = attempt,
                                error = %e,
                                "Saga retry failed"
                            );
                            break 'retry;
                        }
                    }
                } else {
                    // No event query router - can't refresh state for retry
                    warn!(
                        correlation_id = %correlation_id,
                        "Cannot retry: no event_query_router configured"
                    );
                    break 'retry;
                }

                // Wait before next retry
                let delay = retry_config.delay_for_attempt(attempt);
                tokio::time::sleep(delay).await;
                attempt += 1;
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
