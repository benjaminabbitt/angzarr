//! Process manager orchestration abstraction.
//!
//! Process managers (PMs) coordinate multi-domain workflows by correlating events
//! across different aggregates. Unlike sagas, PMs are **stateful** — they maintain
//! their own event stream to track workflow progress.
//!
//! # PM vs Saga
//!
//! | Aspect | Saga | Process Manager |
//! |--------|------|-----------------|
//! | State | Stateless (per-event) | Stateful (persists progress) |
//! | Input | Single domain events | Multi-domain events (via correlation_id) |
//! | Identity | None (ephemeral) | correlation_id IS the PM root |
//! | Recovery | Replay event | Resume from persisted PM state |
//!
//! # Correlation ID as PM Root
//!
//! The correlation_id serves double duty: it identifies both the cross-domain
//! workflow AND the PM's aggregate root. This means:
//!
//! - `fetcher.fetch_by_correlation(pm_domain, correlation_id)` returns the PM's own state
//! - All PM events are stored under `(pm_domain, correlation_id)` as root
//! - Commands rejected route back via `saga_origin.triggering_aggregate.root = correlation_id`
//!
//! # Execution Flow
//!
//! 1. **Fetch PM state**: Load existing workflow progress by correlation_id
//! 2. **Prepare**: PM declares additional destination aggregates to fetch
//! 3. **Handle**: PM produces commands + its own state events
//! 4. **Persist PM events**: Store PM state changes (retries on conflict)
//! 5. **Execute commands**: Send to target aggregates (no retry here)
//!
//! PM event persistence is retried but command execution is not — commands may
//! succeed/fail independently, and the PM can observe outcomes via Notifications.
//!
//! # Module Structure
//!
//! - `local/`: in-process PM handler calls (standalone mode)
//! - `grpc/`: remote gRPC PM client calls (distributed mode)

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
use super::FactExecutor;

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
    /// Facts to inject to other aggregates.
    pub facts: Vec<EventBook>,
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
/// # Why Manual Retry Loop (Not RetryableOperation)
///
/// Unlike sagas, PM retry is simpler: we retry the ENTIRE flow from PM state fetch.
/// Sagas use `RetryableOperation` with selective destination caching because they
/// may target multiple unrelated aggregates. PMs are different:
///
/// - PM state is always re-fetched (it's the thing that might have conflicted)
/// - The retry boundary is "PM events persisted" — once that succeeds, we're done
/// - Commands are fire-and-forget with compensation (no retry needed)
///
/// A manual loop with `delays.next()` is clearer than shoehorning this into the
/// saga retry pattern.
///
/// # Ordering Invariant: PM Events Before Commands
///
/// PM events MUST persist before executing commands. This ensures:
///
/// 1. **Crash recovery**: If we crash after persisting PM events but before
///    commands, the PM state records what we intended to do. On restart, we
///    can either retry commands or detect duplicates.
///
/// 2. **Compensation routing**: If a command fails, the PM receives a Notification.
///    The PM state must already reflect that we attempted this command so the
///    compensation handler has context.
///
/// If we reversed the order (commands first), a crash between command success
/// and PM event persistence would leave the PM state inconsistent.
///
/// # Flow Summary
///
/// 1. Fetch PM state by correlation_id (PM root = correlation_id by design)
/// 2. Prepare: PM declares additional destinations needed
/// 3. Fetch destination event books
/// 4. Handle: PM produces commands + PM events + facts
/// 5. Persist PM events (retries on sequence conflict)
/// 6. Execute commands with saga_origin stamped for compensation routing
/// 7. Inject facts into target aggregates
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(name = "pm.orchestrate", skip_all, fields(%pm_name, %pm_domain, %correlation_id))]
pub async fn orchestrate_pm(
    ctx: &dyn ProcessManagerContext,
    fetcher: &dyn DestinationFetcher,
    executor: &dyn CommandExecutor,
    fact_executor: Option<&dyn FactExecutor>,
    trigger: &EventBook,
    pm_name: &str,
    pm_domain: &str,
    correlation_id: &str,
    backoff: ExponentialBuilder,
) -> Result<(), BusError> {
    let trigger_domain = trigger
        .cover
        .as_ref()
        .map(|c| c.domain.as_str())
        .unwrap_or("unknown");

    debug!(
        %trigger_domain,
        "Processing event in process manager"
    );

    // Manual retry loop for PM event persistence. We retry from PM state fetch
    // because sequence conflicts mean another instance updated the PM state
    // concurrently — we need to re-read it before retrying.
    let mut delays = backoff.build();
    let mut attempt = 0u32;

    loop {
        // Load PM state by correlation_id.
        // Why by correlation_id? The PM's aggregate root IS the correlation_id.
        // This is a design choice: a PM instance is identified by the workflow
        // it coordinates, not by an arbitrary UUID. This simplifies lookups and
        // ensures all events for a workflow flow through one PM instance.
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

        // Persist PM events with retry on sequence conflicts.
        //
        // This is the critical persistence boundary. PM events record:
        // - What commands we intend to send
        // - Current workflow state (phase, accumulated data)
        //
        // Why retry here? Sequence conflicts mean another PM instance (or a replay)
        // updated this workflow concurrently. We must re-fetch PM state and re-run
        // the handle phase with fresh context.
        //
        // Why NOT retry command execution? Commands are idempotent at the aggregate
        // level (sequences prevent duplicate application). If a command fails with
        // sequence conflict, the aggregate saw a concurrent write — the PM will
        // receive a Notification and can decide whether to retry or compensate.
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
                            crate::utils::retry::log_retry_attempt(
                                &format!("pm:{pm_name}"),
                                attempt,
                                &reason,
                                delay,
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                        None => {
                            crate::utils::retry::log_retry_exhausted(
                                &format!("pm:{pm_name}"),
                                attempt,
                                &reason,
                            );
                            return Err(BusError::Publish(reason));
                        }
                    },
                    CommandOutcome::Rejected(reason) => {
                        crate::utils::retry::log_fatal_error(
                            &format!("pm:{pm_name}"),
                            attempt,
                            &reason,
                        );
                        return Err(BusError::Publish(reason));
                    }
                }
            }
        }

        // Execute commands produced by process manager.
        //
        // At this point, PM events are persisted (the "point of no return").
        // Command execution is fire-and-forget:
        // - Success: great, workflow progresses
        // - Sequence conflict: aggregate was modified concurrently; PM receives
        //   Notification and can retry or compensate
        // - Rejected: aggregate refused the command; PM receives Notification
        //   and invokes compensation handler
        //
        // We do NOT retry command execution here because:
        // 1. PM events are already persisted — retrying the whole flow would
        //    create duplicate PM events
        // 2. The PM's job is to observe outcomes and react, not guarantee delivery
        // 3. Compensation is the PM's mechanism for handling failures
        execute_pm_commands(
            ctx,
            executor,
            response.commands,
            correlation_id,
            pm_name,
            pm_domain,
        )
        .await;

        // Inject facts into target aggregates.
        //
        // Facts are events emitted by the PM that are injected directly into target
        // aggregates without command handling. The coordinator stamps sequence numbers
        // on receipt based on the aggregate's current state.
        //
        // Facts must have `external_id` set in their Cover for idempotent handling.
        // Fact injection failure fails the entire PM operation — facts are not
        // best-effort, they're part of the transaction.
        if let Some(fact_exec) = fact_executor {
            for fact in response.facts {
                let domain = fact
                    .cover
                    .as_ref()
                    .map(|c| c.domain.as_str())
                    .unwrap_or("unknown");
                debug!(%domain, "Injecting fact from PM");

                fact_exec
                    .inject(fact)
                    .await
                    .map_err(|e| BusError::Publish(format!("PM fact injection failed: {e}")))?;
            }
        }

        // Exit retry loop. PM events are persisted, commands are dispatched.
        // The workflow continues asynchronously via Notifications.
        break;
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
        external_id: String::new(),
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
