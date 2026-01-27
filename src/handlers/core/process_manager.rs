//! Process Manager event handler.
//!
//! Receives events from the event bus and forwards them to process manager
//! gRPC services. Process managers coordinate long-running workflows across
//! multiple aggregates using correlation_id to track related events.
//!
//! Unlike sagas (stateless, single event → commands), process managers:
//! - Maintain persistent state keyed by correlation_id (event-sourced)
//! - Receive events from ANY aggregate with matching correlation_id
//! - Make decisions based on accumulated state across the workflow
//! - Persist their own events to their own domain
//!
//! Two-phase protocol:
//! 1. Prepare: PM declares additional destinations needed (beyond trigger)
//! 2. Handle: PM receives trigger + PM state + destinations, produces commands + PM events
//!
//! All outputs preserve the original correlation_id for streaming.

use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::bus::{BusError, EventBus, EventHandler};
use crate::handlers::core::saga::{CommandRouter, EventQueryRouter};
use crate::proto::process_manager_client::ProcessManagerClient;
use crate::proto::{
    CommandBook, Cover, EventBook, ProcessManagerHandleRequest, ProcessManagerPrepareRequest,
};
use crate::utils::retry::{is_retryable_status, RetryConfig};

/// Event handler that forwards events to a process manager gRPC service.
///
/// Process managers coordinate long-running workflows by:
/// 1. Receiving events from any aggregate with matching correlation_id
/// 2. Maintaining event-sourced state in their own domain
/// 3. Producing commands to drive the workflow forward
///
/// Two-phase protocol per event:
/// - Prepare: PM declares which additional destinations it needs
/// - Handle: PM receives full context, produces commands + process events
pub struct ProcessManagerEventHandler {
    /// gRPC client for the process manager service.
    client: Arc<Mutex<ProcessManagerClient<tonic::transport::Channel>>>,
    /// Process manager domain name (for state queries).
    process_domain: String,
    /// Router for querying event books by correlation_id.
    event_query_router: EventQueryRouter,
    /// Router for executing commands on aggregates.
    command_router: CommandRouter,
    /// Event bus for publishing events.
    publisher: Arc<dyn EventBus>,
    /// Retry configuration for sequence conflicts.
    retry_config: RetryConfig,
}

impl ProcessManagerEventHandler {
    /// Create a new process manager event handler.
    ///
    /// # Arguments
    /// * `client` - gRPC client for the process manager service
    /// * `process_domain` - Domain name for the process manager's own state
    /// * `event_query_router` - Router for fetching EventBooks by correlation_id
    /// * `command_router` - Router for executing commands on aggregates
    /// * `publisher` - Event bus for publishing events
    pub fn new(
        client: ProcessManagerClient<tonic::transport::Channel>,
        process_domain: String,
        event_query_router: EventQueryRouter,
        command_router: CommandRouter,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            process_domain,
            event_query_router,
            command_router,
            publisher,
            retry_config: RetryConfig::for_saga_commands(),
        }
    }

    /// Create with custom retry configuration.
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }
}

impl EventHandler for ProcessManagerEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let client = self.client.clone();
        let process_domain = self.process_domain.clone();
        let event_query_router = self.event_query_router.clone();
        let command_router = self.command_router.clone();
        let _publisher = self.publisher.clone();
        let retry_config = self.retry_config.clone();

        Box::pin(async move {
            let book_owned = (*book).clone();
            let correlation_id = book_owned
                .cover
                .as_ref()
                .map(|c| c.correlation_id.clone())
                .unwrap_or_default();

            // Process managers require correlation_id to track workflow
            if correlation_id.is_empty() {
                debug!("Event has no correlation_id, skipping process manager");
                return Ok(());
            }

            let trigger_domain = book_owned
                .cover
                .as_ref()
                .map(|c| c.domain.clone())
                .unwrap_or_else(|| "unknown".to_string());

            debug!(
                correlation_id = %correlation_id,
                trigger_domain = %trigger_domain,
                process_domain = %process_domain,
                events = book_owned.pages.len(),
                "Processing event in process manager"
            );

            // Retry loop for sequence conflicts
            let mut attempt = 0u32;
            loop {
                // Load trigger domain state by correlation_id
                let trigger_cover = Cover {
                    domain: trigger_domain.clone(),
                    root: None,
                    correlation_id: correlation_id.clone(),
                };
                let trigger_state = match event_query_router.fetch(&trigger_cover).await {
                    Ok(eb) => eb,
                    Err(e) => {
                        warn!(
                            correlation_id = %correlation_id,
                            domain = %trigger_domain,
                            error = %e,
                            "Failed to fetch trigger state, using incoming event"
                        );
                        book_owned.clone()
                    }
                };

                // Load PM state by correlation_id
                let pm_cover = Cover {
                    domain: process_domain.clone(),
                    root: None,
                    correlation_id: correlation_id.clone(),
                };
                let pm_state = match event_query_router.fetch(&pm_cover).await {
                    Ok(eb) => Some(eb),
                    Err(e) => {
                        debug!(
                            correlation_id = %correlation_id,
                            domain = %process_domain,
                            error = %e,
                            "No existing PM state (new workflow)"
                        );
                        None
                    }
                };

                // Phase 1: Call Prepare to get additional destinations
                let mut client = client.lock().await;
                let prepare_request = ProcessManagerPrepareRequest {
                    trigger: Some(trigger_state.clone()),
                    process_state: pm_state.clone(),
                };

                let prepare_response = match client.prepare(prepare_request).await {
                    Ok(resp) => resp.into_inner(),
                    Err(e) => {
                        error!(
                            correlation_id = %correlation_id,
                            error = %e,
                            "ProcessManager.Prepare failed"
                        );
                        return Err(BusError::Grpc(e));
                    }
                };

                let destination_covers = prepare_response.destinations;
                debug!(
                    correlation_id = %correlation_id,
                    destinations = destination_covers.len(),
                    "ProcessManager.Prepare returned destinations"
                );

                // Fetch additional destinations
                let mut destinations = Vec::with_capacity(destination_covers.len());
                for cover in &destination_covers {
                    match event_query_router.fetch(cover).await {
                        Ok(event_book) => destinations.push(event_book),
                        Err(e) => {
                            warn!(
                                correlation_id = %correlation_id,
                                domain = %cover.domain,
                                error = %e,
                                "Failed to fetch destination, skipping"
                            );
                        }
                    }
                }

                // Phase 2: Call Handle with full context
                let handle_request = ProcessManagerHandleRequest {
                    trigger: Some(trigger_state),
                    process_state: pm_state,
                    destinations,
                };

                let handle_response = match client.handle(handle_request).await {
                    Ok(resp) => resp.into_inner(),
                    Err(e) => {
                        error!(
                            correlation_id = %correlation_id,
                            error = %e,
                            "ProcessManager.Handle failed"
                        );
                        return Err(BusError::Grpc(e));
                    }
                };

                debug!(
                    correlation_id = %correlation_id,
                    commands = handle_response.commands.len(),
                    has_process_events = handle_response.process_events.is_some(),
                    "ProcessManager.Handle returned response"
                );

                // Persist process events to PM's own domain via command router
                // Process events are persisted as the PM aggregate's state
                if let Some(process_events) = handle_response.process_events {
                    if !process_events.pages.is_empty() {
                        // Build a command to persist the PM events
                        // The PM domain uses a special "persist" command pattern
                        let pm_command = CommandBook {
                            cover: Some(Cover {
                                domain: process_domain.clone(),
                                root: process_events.cover.as_ref().and_then(|c| c.root.clone()),
                                correlation_id: correlation_id.clone(),
                            }),
                            pages: vec![], // Events passed separately
                            saga_origin: None,
                        };

                        match command_router.execute(pm_command).await {
                            Ok(_) => {
                                info!(
                                    correlation_id = %correlation_id,
                                    domain = %process_domain,
                                    events = process_events.pages.len(),
                                    "PM events persisted successfully"
                                );
                            }
                            Err(e) if is_retryable_status(&e) && attempt < retry_config.max_retries => {
                                warn!(
                                    correlation_id = %correlation_id,
                                    attempt = attempt,
                                    error = %e,
                                    "Sequence conflict persisting PM events, retrying"
                                );
                                drop(client); // Release lock before retry
                                let delay = retry_config.delay_for_attempt(attempt);
                                tokio::time::sleep(delay).await;
                                attempt += 1;
                                continue; // Retry the whole flow
                            }
                            Err(e) => {
                                error!(
                                    correlation_id = %correlation_id,
                                    domain = %process_domain,
                                    error = %e,
                                    "Failed to persist PM events (non-retryable)"
                                );
                                return Err(BusError::Grpc(e));
                            }
                        }
                    }
                }

                // Execute commands produced by process manager
                for mut command_book in handle_response.commands {
                    // Ensure correlation_id is propagated
                    if let Some(ref mut cover) = command_book.cover {
                        if cover.correlation_id.is_empty() {
                            cover.correlation_id = correlation_id.clone();
                        }
                    }

                    let cmd_domain = command_book
                        .cover
                        .as_ref()
                        .map(|c| c.domain.as_str())
                        .unwrap_or("unknown");

                    debug!(
                        correlation_id = %correlation_id,
                        domain = %cmd_domain,
                        "Executing process manager command"
                    );

                    match command_router.execute(command_book.clone()).await {
                        Ok(sync_resp) => {
                            debug!(
                                correlation_id = %correlation_id,
                                domain = %cmd_domain,
                                has_events = sync_resp.events.is_some(),
                                "Process manager command executed successfully"
                            );
                        }
                        Err(e) if is_retryable_status(&e) => {
                            warn!(
                                correlation_id = %correlation_id,
                                domain = %cmd_domain,
                                attempt = attempt,
                                error = %e,
                                "Command sequence conflict, will retry"
                            );
                            // For command conflicts, we don't retry the whole PM flow,
                            // just log and continue - the PM will be triggered again
                            // when events from other domains arrive
                        }
                        Err(e) => {
                            error!(
                                correlation_id = %correlation_id,
                                domain = %cmd_domain,
                                error = %e,
                                "Process manager command failed (non-retryable)"
                            );
                            // Continue with other commands
                        }
                    }
                }

                // Success - exit retry loop
                break;
            }

            Ok(())
        })
    }
}
