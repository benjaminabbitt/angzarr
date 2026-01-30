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
use crate::proto::{CommandBook, Cover, EventBook};

use super::command::{CommandExecutor, CommandOutcome};
use super::destination::DestinationFetcher;

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
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>>;

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
/// 1. Fetch trigger state by correlation_id
/// 2. Fetch PM state by correlation_id
/// 3. Prepare: PM declares additional destinations
/// 4. Fetch destination event books
/// 5. Handle: PM produces commands + PM events
/// 6. Persist PM events (retries on sequence conflict)
/// 7. Execute commands with correlation_id propagation
#[tracing::instrument(name = "pm.orchestrate", skip_all, fields(%pm_domain, %correlation_id))]
pub async fn orchestrate_pm(
    ctx: &dyn ProcessManagerContext,
    fetcher: &dyn DestinationFetcher,
    executor: &dyn CommandExecutor,
    trigger: &EventBook,
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
        // Load trigger domain state by correlation_id
        let trigger_state = fetcher
            .fetch_by_correlation(trigger_domain, correlation_id)
            .await
            .unwrap_or_else(|| {
                warn!(
                    %trigger_domain,
                    "Failed to fetch trigger state, using incoming event"
                );
                trigger.clone()
            });

        // Load PM state by correlation_id
        let pm_state = fetcher
            .fetch_by_correlation(pm_domain, correlation_id)
            .await;

        if pm_state.is_none() {
            debug!("No existing PM state (new workflow)");
        }

        // Phase 1: Prepare — get additional destination covers
        let destination_covers = ctx
            .prepare(&trigger_state, pm_state.as_ref())
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;

        debug!(
            destinations = destination_covers.len(),
            "ProcessManager.Prepare returned destinations"
        );

        // Fetch additional destinations
        let destinations =
            super::shared::fetch_destinations(fetcher, &destination_covers, correlation_id).await;

        // Phase 2: Handle — produce commands + PM events
        let response = ctx
            .handle(&trigger_state, pm_state.as_ref(), &destinations)
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
                    CommandOutcome::Retryable { reason, .. } => {
                        match delays.next() {
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
                        }
                    }
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
        super::shared::execute_commands(executor, response.commands, correlation_id).await;

        // Success — exit retry loop
        break;
    }

    #[cfg(feature = "otel")]
    {
        use crate::utils::metrics::{self, PM_DURATION};
        PM_DURATION.record(start.elapsed().as_secs_f64(), &[
            metrics::component_attr("process_manager"),
            metrics::name_attr(pm_domain),
        ]);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
