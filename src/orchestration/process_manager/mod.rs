//! Process manager orchestration abstraction.
//!
//! `ProcessManagerContext` defines PM-specific operations (prepare, handle, persist).
//! `orchestrate_pm` implements the full PM flow with retry on sequence conflicts.
//!
//! - `local/`: in-process PM handler calls
//! - `grpc/`: remote gRPC PM client calls

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use async_trait::async_trait;
use backon::{BackoffBuilder, ExponentialBuilder};
use tracing::{debug, error, info, warn};

use crate::bus::BusError;
use crate::proto::{CommandBook, Cover, EventBook, SagaCommandOrigin, Uuid as ProtoUuid};

use super::command::{CommandExecutor, CommandOutcome};
use super::destination::DestinationFetcher;

/// Response from a process manager's prepare phase.
pub struct PmPrepareResponse {
    /// Additional aggregates needed beyond trigger.
    pub destinations: Vec<Cover>,
}

/// Response from a process manager's handle phase.
pub struct PmHandleResponse {
    /// Commands to execute on aggregates.
    pub commands: Vec<CommandBook>,
    /// Optional PM events to persist to the PM's own domain.
    pub process_events: Option<EventBook>,
}

/// PM-specific operations abstracted over transport.
///
/// Implementations provide prepare/handle via in-process handler (local)
/// or gRPC client (distributed). PM event persistence differs significantly:
/// local writes to event store + re-reads + publishes; gRPC routes through
/// CommandExecutor.
#[async_trait]
pub trait ProcessManagerContext: Send + Sync {
    /// Phase 1: PM declares additional destinations needed beyond trigger + PM state.
    async fn prepare(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
    ) -> Result<PmPrepareResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Phase 2: PM produces commands + process events given trigger, PM state, and destinations.
    async fn handle(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Persist PM events to the PM's own domain.
    async fn persist_pm_events(
        &self,
        process_events: &EventBook,
        correlation_id: &str,
    ) -> CommandOutcome;

    /// Handle a rejected command produced by this PM.
    ///
    /// Called when a command produced by this PM is rejected by the target aggregate.
    /// Implementations should invoke `handle_revocation()` on the PM handler and
    /// persist any resulting PM events.
    ///
    /// Default implementation logs the rejection. Override in implementations
    /// that have access to compensation handlers.
    async fn on_command_rejected(
        &self,
        _command: &CommandBook,
        _reason: &str,
        _correlation_id: &str,
    ) {
        // Default: log only, no compensation
        tracing::error!(
            reason = %_reason,
            "PM command rejected (no compensation path configured)"
        );
    }
}

/// Factory for creating per-invocation PM contexts.
///
/// Implementations capture long-lived dependencies and produce a fresh
/// `ProcessManagerContext` for each event. Also provides the PM domain
/// needed by `orchestrate_pm`.
pub trait PMContextFactory: Send + Sync {
    /// Create a PM context for one invocation.
    fn create(&self) -> Box<dyn ProcessManagerContext>;

    /// The domain this process manager owns (for PM event persistence).
    fn pm_domain(&self) -> &str;

    /// The name of this process manager (used for metrics and tracing).
    fn name(&self) -> &str;
}

/// Full process manager orchestration with retry on sequence conflicts.
///
/// Flow:
/// 1. Fetch PM state by correlation_id (PM root = correlation_id by design)
/// 2. Prepare: PM declares additional destinations needed
/// 3. Fetch destination event books
/// 4. Handle: PM produces commands + PM events
/// 5. Persist PM events (retries on sequence conflict)
/// 6. Execute commands with saga_origin stamped for compensation routing
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(name = "pm.orchestrate", skip_all, fields(%pm_name, %pm_domain, %correlation_id))]
pub async fn orchestrate_pm(
    ctx: &dyn ProcessManagerContext,
    fetcher: &dyn DestinationFetcher,
    executor: &dyn CommandExecutor,
    trigger: &EventBook,
    pm_name: &str,
    pm_domain: &str,
    correlation_id: &str,
    backoff: ExponentialBuilder,
) -> Result<(), BusError> {
    #[cfg(feature = "otel")]
    let start = std::time::Instant::now();

    let trigger_domain = trigger
        .cover
        .as_ref()
        .map(|c| c.domain.as_str())
        .unwrap_or("unknown");

    debug!(
        %trigger_domain,
        "Processing event in process manager"
    );

    let mut delays = backoff.build();
    let mut attempt = 0u32;

    loop {
        // Load PM state by correlation_id (PM root = correlation_id by design)
        let pm_state = fetcher
            .fetch_by_correlation(pm_domain, correlation_id)
            .await;

        if pm_state.is_none() {
            debug!("No existing PM state (new workflow)");
        }

        // Phase 1: Prepare — get additional destination covers
        let prepare_response = ctx
            .prepare(trigger, pm_state.as_ref())
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;

        let destination_covers = prepare_response.destinations;

        debug!(
            destinations = destination_covers.len(),
            "ProcessManager.Prepare returned destinations"
        );

        // Fetch additional destinations
        let destinations =
            super::shared::fetch_destinations(fetcher, &destination_covers, correlation_id).await;

        // Phase 2: Handle — produce commands + PM events
        // Use original trigger (from bus) so PM sees the actual triggering event pages
        // PM state provides workflow context; destinations provide aggregate state
        let response = ctx
            .handle(trigger, pm_state.as_ref(), &destinations)
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;

        debug!(
            commands = response.commands.len(),
            has_process_events = response.process_events.is_some(),
            "ProcessManager.Handle returned response"
        );

        // Persist PM events with retry on sequence conflicts
        if let Some(ref process_events) = response.process_events {
            if !process_events.pages.is_empty() {
                match ctx.persist_pm_events(process_events, correlation_id).await {
                    CommandOutcome::Success(_) => {
                        info!(
                            events = process_events.pages.len(),
                            "PM events persisted successfully"
                        );
                    }
                    CommandOutcome::Retryable { reason, .. } => match delays.next() {
                        Some(delay) => {
                            warn!(
                                attempt,
                                error = %reason,
                                "Sequence conflict persisting PM events, retrying"
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                        None => {
                            error!(
                                error = %reason,
                                "Failed to persist PM events (retries exhausted)"
                            );
                            return Err(BusError::Publish(reason));
                        }
                    },
                    CommandOutcome::Rejected(reason) => {
                        error!(
                            error = %reason,
                            "Failed to persist PM events"
                        );
                        return Err(BusError::Publish(reason));
                    }
                }
            }
        }

        // Execute commands produced by process manager
        // PM handler must set correct sequences on commands from destination.next_sequence()
        // saga_origin is stamped so compensation routes back to PM
        execute_pm_commands(
            ctx,
            executor,
            response.commands,
            correlation_id,
            pm_name,
            pm_domain,
        )
        .await;

        // Success — exit retry loop
        break;
    }

    #[cfg(feature = "otel")]
    {
        use crate::utils::metrics::{self, PM_DURATION};
        PM_DURATION.record(
            start.elapsed().as_secs_f64(),
            &[
                metrics::component_attr("process_manager"),
                metrics::name_attr(pm_domain),
            ],
        );
    }

    Ok(())
}

/// Execute PM commands with saga_origin stamped for compensation routing.
///
/// Stamps each command with `saga_origin` pointing to the PM itself, so that
/// if a command is rejected, the compensation Notification routes back to the
/// PM through the standard aggregate coordinator infrastructure.
///
/// PMs are aggregates — they receive Notifications the same way aggregates do.
async fn execute_pm_commands(
    ctx: &dyn ProcessManagerContext,
    executor: &dyn CommandExecutor,
    mut commands: Vec<CommandBook>,
    correlation_id: &str,
    pm_name: &str,
    pm_domain: &str,
) {
    use super::shared::fill_correlation_id;
    fill_correlation_id(&mut commands, correlation_id);

    // Build PM cover for saga_origin — PM is the triggering aggregate
    // PM root = correlation_id by design (PM is identified by the workflow it coordinates)
    let pm_cover = Cover {
        domain: pm_domain.to_string(),
        root: Some(ProtoUuid {
            value: uuid::Uuid::parse_str(correlation_id)
                .unwrap_or_else(|_| uuid::Uuid::nil())
                .as_bytes()
                .to_vec(),
        }),
        correlation_id: correlation_id.to_string(),
        edition: None,
    };

    // Stamp saga_origin on all PM commands so compensation routes back to PM
    for cmd in &mut commands {
        if cmd.saga_origin.is_none() {
            cmd.saga_origin = Some(SagaCommandOrigin {
                saga_name: pm_name.to_string(),
                triggering_aggregate: Some(pm_cover.clone()),
                triggering_event_sequence: 0,
            });
        }
    }

    for command_book in commands {
        let cmd_domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| "unknown".to_string());

        debug!(
            domain = %cmd_domain,
            "Executing PM command"
        );

        match executor.execute(command_book.clone()).await {
            CommandOutcome::Success(cmd_response) => {
                debug!(
                    domain = %cmd_domain,
                    has_events = cmd_response.events.is_some(),
                    "PM command executed successfully"
                );
            }
            CommandOutcome::Retryable { reason, .. } => {
                warn!(
                    domain = %cmd_domain,
                    error = %reason,
                    "PM command sequence conflict (will be retried)"
                );
            }
            CommandOutcome::Rejected(reason) => {
                error!(
                    domain = %cmd_domain,
                    error = %reason,
                    "PM command rejected, invoking compensation"
                );
                ctx.on_command_rejected(&command_book, &reason, correlation_id)
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests;
