//! Saga orchestration abstraction.
//!
//! Sagas are **pure translators**: they receive source events and produce commands
//! for target domains. They are stateless — each event is processed independently
//! with no memory of previous events. This enables horizontal scaling and simple recovery.
//!
//! # Execution Model
//!
//! Sagas receive only source events — NO destination state. The framework handles:
//!
//! 1. **Sequence stamping**: Commands have `angzarr_deferred`, framework stamps
//!    explicit sequences on delivery.
//!
//! 2. **Delivery retry**: On sequence conflict, framework retries command delivery
//!    with fresh sequence (NOT saga re-execution).
//!
//! 3. **Provenance tracking**: `angzarr_deferred` links commands to source events
//!    for compensation routing and idempotency.
//!
//! # Retry Strategy
//!
//! When commands fail due to sequence conflicts, we retry at the delivery level
//! with exponential backoff. The saga is NOT re-executed — commands are produced
//! once, and the framework handles delivery retries.
//!
//! # Module Structure
//!
//! - `local/`: in-process saga handler calls (standalone mode)
//! - `grpc/`: remote gRPC saga client calls (distributed mode)

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use backon::ExponentialBuilder;
use tracing::{debug, error, warn};

use crate::bus::BusError;
use crate::bus::CommandBus;
use crate::proto::{
    page_header::SequenceType, AngzarrDeferredSequence, CommandBook, Cover, EventBook, PageHeader,
    SagaResponse, SyncMode,
};
use crate::proto_ext::CoverExt;
use crate::utils::retry::{run_with_retry, RetryOutcome, RetryableOperation};

use super::command::{CommandExecutor, CommandOutcome};
use super::destination::DestinationFetcher;
use super::FactExecutor;

/// Validator for saga output domain routing.
pub type OutputDomainValidator = dyn Fn(&CommandBook) -> Result<(), String> + Send + Sync;

/// Factory for creating per-invocation saga contexts.
///
/// Implementations capture long-lived dependencies (clients, handlers,
/// executors) and produce a fresh `SagaRetryContext` for each event.
/// Local and gRPC modes provide different implementations.
pub trait SagaContextFactory: Send + Sync {
    /// Create a saga context for processing the given source event book.
    fn create(&self, source: Arc<EventBook>) -> Box<dyn SagaRetryContext>;

    /// The name of this saga (used for metrics and tracing).
    fn name(&self) -> &str;
}

/// Operations needed by the saga orchestration.
///
/// Each transport mode implements this trait to provide saga-specific
/// invocation and compensation. One instance per saga invocation —
/// captures the per-invocation context (source event book, saga handler, etc.)
///
/// The new model has sagas as pure translators:
/// - Saga receives only source events (no destination state)
/// - Saga produces commands with deferred sequences
/// - Framework stamps explicit sequences on delivery
/// - Framework retries delivery on conflict (not saga re-execution)
#[async_trait]
pub trait SagaRetryContext: Send + Sync {
    /// Execute saga translation: source events → commands + facts.
    ///
    /// Returns commands (with angzarr_deferred) to deliver and events (facts)
    /// to inject. The saga does NOT set explicit sequences — the framework
    /// stamps them on delivery.
    async fn handle(&self) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Handle a permanently rejected command (compensation, logging, etc.)
    async fn on_command_rejected(&self, command: &CommandBook, reason: &str);

    /// Cover of the source event that triggered this saga invocation.
    ///
    /// Used to populate `angzarr_deferred` source on outgoing commands,
    /// enabling rejection routing back to the originating aggregate.
    fn source_cover(&self) -> Option<&Cover>;

    /// Max sequence number from the source EventBook.
    ///
    /// Used as the default `source_seq` in `angzarr_deferred` when the saga
    /// doesn't explicitly set it. Represents "processed up to this point".
    ///
    /// Sagas that need precise per-event tracking should set `source_seq`
    /// explicitly on each command's `PageHeader.angzarr_deferred`.
    fn source_max_sequence(&self) -> u32;
}

/// State for retryable saga command delivery.
///
/// Commands have `angzarr_deferred` set — the executor handles converting this
/// to explicit sequences on delivery and retrying at the delivery level.
///
/// This struct tracks which commands have been delivered and handles rejection
/// callbacks for permanently failed commands.
#[cfg_attr(not(feature = "otel"), allow(dead_code))]
struct SagaOperation<'a> {
    context: &'a dyn SagaRetryContext,
    executor: &'a dyn CommandExecutor,
    /// Command bus for async command publishing.
    /// When `sync_mode == Async` and this is `Some`, commands are published
    /// to the bus (fire-and-forget) instead of executed directly.
    command_bus: Option<&'a dyn CommandBus>,
    saga_name: &'a str,
    correlation_id: &'a str,
    /// Sync mode for command execution.
    /// ASYNC: commands published to bus (fire-and-forget), results via RejectionNotification.
    /// CASCADE: commands executed synchronously with no bus publishing.
    /// SIMPLE: commands executed synchronously with bus publishing.
    sync_mode: SyncMode,
    commands: Vec<CommandBook>,
    /// Events (facts) to inject after all commands succeed.
    #[allow(dead_code)]
    events: Vec<EventBook>,
    /// Tracks which domains had sequence conflicts for retry logging.
    failed_domains: HashSet<String>,
}

#[async_trait]
impl<'a> RetryableOperation for SagaOperation<'a> {
    type Success = Vec<EventBook>;
    type Failure = String;

    fn name(&self) -> &str {
        "saga_command_execution"
    }

    async fn try_execute(&mut self) -> RetryOutcome<Self::Success, Self::Failure> {
        // Clear failed_domains at the start of each attempt. This is intentional:
        // we only care about which domains failed THIS attempt, not previous ones.
        // The cache persists across attempts; failed_domains is per-attempt tracking.
        self.failed_domains.clear();

        for command in &self.commands {
            let mut command = command.clone();
            if let Some(ref mut cover) = command.cover {
                if cover.correlation_id.is_empty() {
                    cover.correlation_id = self.correlation_id.to_string();
                }
            }

            let domain = command.domain().to_string();

            // ASYNC mode: publish to command bus (fire-and-forget).
            // Results come back via RejectionNotification through the event bus.
            // No retry loop needed — the command handler will handle sequence
            // conflicts and rejection routing.
            if self.sync_mode == SyncMode::Async {
                if let Some(bus) = self.command_bus {
                    match bus.publish(Arc::new(command)).await {
                        Ok(()) => {
                            debug!(%domain, "Saga command published to bus (async)");
                        }
                        Err(e) => {
                            error!(%domain, error = %e, "Failed to publish command to bus");
                            // Bus publish failure is not retryable — infrastructure error
                            return RetryOutcome::Fatal(format!("Command bus publish failed: {e}"));
                        }
                    }
                    continue;
                }
                // Fall through to direct execution if no bus configured
            }

            // SIMPLE/CASCADE mode: execute synchronously
            match self.executor.execute(command.clone(), self.sync_mode).await {
                CommandOutcome::Success(_) => {
                    debug!(%domain, "Saga command executed successfully");
                }
                CommandOutcome::Retryable { reason, .. } => {
                    warn!(%domain, error = %reason, "Sequence conflict, will retry with fresh state");
                    self.failed_domains.insert(domain);
                }
                CommandOutcome::Rejected(reason) => {
                    error!(%domain, error = %reason, "Saga command rejected (non-retryable)");
                    self.context.on_command_rejected(&command, &reason).await;
                }
            }
        }

        if !self.failed_domains.is_empty() {
            RetryOutcome::Retryable("Sequence conflict".to_string())
        } else {
            RetryOutcome::Success(vec![])
        }
    }

    async fn prepare_for_retry(&mut self) -> Result<(), Self::Failure> {
        // Record retry metric
        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{name_attr, SAGA_RETRY_TOTAL};
            SAGA_RETRY_TOTAL.add(1, &[name_attr(self.saga_name)]);
        }

        // In the new model, sagas are NOT re-executed on retry.
        // Commands are produced once with angzarr_deferred sequences.
        // Retry happens at the delivery level (executor handles sequence stamping).
        //
        // This prepare_for_retry just clears the failed domains for the next attempt.
        self.failed_domains.clear();

        Ok(())
    }
}

/// Builder for saga command delivery with retry.
///
/// Commands have angzarr_deferred set — the executor handles sequence stamping
/// and delivery-level retry.
struct SagaRetryBuilder<'a> {
    context: &'a dyn SagaRetryContext,
    executor: &'a dyn CommandExecutor,
    command_bus: Option<&'a dyn CommandBus>,
    saga_name: &'a str,
    correlation_id: &'a str,
    sync_mode: SyncMode,
    commands: Vec<CommandBook>,
    events: Vec<EventBook>,
    backoff: ExponentialBuilder,
}

impl<'a> SagaRetryBuilder<'a> {
    fn new(
        context: &'a dyn SagaRetryContext,
        executor: &'a dyn CommandExecutor,
        saga_name: &'a str,
        correlation_id: &'a str,
        sync_mode: SyncMode,
    ) -> Self {
        Self {
            context,
            executor,
            command_bus: None,
            saga_name,
            correlation_id,
            sync_mode,
            commands: Vec::new(),
            events: Vec::new(),
            backoff: ExponentialBuilder::default(),
        }
    }

    fn command_bus(mut self, command_bus: Option<&'a dyn CommandBus>) -> Self {
        self.command_bus = command_bus;
        self
    }

    fn commands(mut self, commands: Vec<CommandBook>) -> Self {
        self.commands = commands;
        self
    }

    fn events(mut self, events: Vec<EventBook>) -> Self {
        self.events = events;
        self
    }

    fn backoff(mut self, backoff: ExponentialBuilder) -> Self {
        self.backoff = backoff;
        self
    }

    /// Deliver saga commands with retry on sequence conflicts.
    #[tracing::instrument(name = "saga.retry", skip_all, fields(saga_name = %self.saga_name, correlation_id = %self.correlation_id))]
    async fn execute(self) {
        if self.commands.is_empty() {
            return;
        }

        let operation = SagaOperation {
            context: self.context,
            executor: self.executor,
            command_bus: self.command_bus,
            saga_name: self.saga_name,
            correlation_id: self.correlation_id,
            sync_mode: self.sync_mode,
            commands: self.commands,
            events: self.events,
            failed_domains: HashSet::new(),
        };

        if let Err(e) = run_with_retry(operation, self.backoff).await {
            error!(error = %e, "Saga execution failed after multiple retries");
        }
    }
}

/// Saga orchestration with delivery-retry model.
///
/// 1. Execute saga translation: source events → commands with angzarr_deferred
/// 2. Stamp provenance (source cover + seq) on commands
/// 3. Validate output domains (if validator provided)
/// 4. Deliver commands with retry on sequence conflict
/// 5. Inject facts into target aggregates
///
/// Sagas are **pure translators** — they receive only source events, not
/// destination state. The framework handles:
/// - Sequence stamping on delivery (converts angzarr_deferred → explicit)
/// - Delivery retry on conflict (not saga re-execution)
/// - Idempotency via angzarr_deferred source info
///
/// `sync_mode` controls how commands are executed:
/// - `Async`: Commands published to bus (fire-and-forget), results via RejectionNotification
/// - `Simple`: Sync execution with bus publishing for downstream sagas
/// - `Cascade`: Full sync chain, no bus publishing
///
/// `command_bus` is required when `sync_mode == Async`. If None and sync_mode is Async,
/// falls back to direct execution.
#[tracing::instrument(name = "saga.orchestrate", skip_all, fields(%saga_name, %correlation_id))]
#[allow(clippy::too_many_arguments)]
pub async fn orchestrate_saga(
    ctx: &dyn SagaRetryContext,
    executor: &dyn CommandExecutor,
    command_bus: Option<&dyn CommandBus>,
    _fetcher: Option<&dyn DestinationFetcher>,
    fact_executor: Option<&dyn FactExecutor>,
    saga_name: &str,
    correlation_id: &str,
    output_domain_validator: Option<&OutputDomainValidator>,
    sync_mode: SyncMode,
    backoff: ExponentialBuilder,
) -> Result<(), BusError> {
    // Phase 1: Execute saga translation
    // Saga receives only source events and produces commands with angzarr_deferred.
    // No destination state fetching — sagas are pure translators.
    let saga_response = ctx
        .handle()
        .await
        .map_err(|e| BusError::Publish(e.to_string()))?;

    let mut commands = saga_response.commands;
    let events = saga_response.events;

    // Stamp angzarr_deferred on commands for provenance and compensation routing:
    //
    // 1. **Compensation routing**: When a command is rejected, the aggregate coordinator
    //    uses angzarr_deferred.source to route the rejection back for compensation.
    //
    // 2. **Traceability**: Links the command to its triggering event for debugging/audit.
    //
    // 3. **Idempotency**: The source + source_seq form the idempotency key for
    //    saga-produced commands, preventing duplicate processing on retry.
    //
    // Stamping strategy (per spec):
    // - Saga already set angzarr_deferred with source_seq → preserve it entirely
    // - Saga set angzarr_deferred but source is None → fill in source Cover, keep source_seq
    // - Saga didn't set angzarr_deferred → use source Cover + source_max_sequence
    let source_cover = ctx.source_cover().cloned();
    let source_max_seq = ctx.source_max_sequence();

    for cmd in &mut commands {
        for page in &mut cmd.pages {
            match page.header.as_ref().and_then(|h| h.sequence_type.as_ref()) {
                Some(SequenceType::AngzarrDeferred(existing)) => {
                    // Saga set angzarr_deferred - fill in source if missing, preserve source_seq
                    if existing.source.is_none() {
                        page.header = Some(PageHeader {
                            sequence_type: Some(SequenceType::AngzarrDeferred(
                                AngzarrDeferredSequence {
                                    source: source_cover.clone(),
                                    source_seq: existing.source_seq,
                                },
                            )),
                        });
                    }
                    // else: saga set everything, don't touch
                }
                _ => {
                    // Saga didn't set angzarr_deferred - use defaults
                    page.header = Some(PageHeader {
                        sequence_type: Some(SequenceType::AngzarrDeferred(
                            AngzarrDeferredSequence {
                                source: source_cover.clone(),
                                source_seq: source_max_seq,
                            },
                        )),
                    });
                }
            }
        }
    }

    debug!(commands = commands.len(), "Saga produced commands");

    // Phase 4: Validate output domains
    if let Some(validator) = output_domain_validator {
        for command_book in &commands {
            if let Err(msg) = validator(command_book) {
                return Err(BusError::SagaFailed {
                    name: "saga".to_string(),
                    message: msg,
                });
            }
        }
    }

    // Phase 4: Execute commands with retry
    // Commands have angzarr_deferred set — the executor handles sequence stamping
    // and retry on conflict at the delivery level.
    SagaRetryBuilder::new(ctx, executor, saga_name, correlation_id, sync_mode)
        .command_bus(command_bus)
        .commands(commands)
        .events(events.clone())
        .backoff(backoff)
        .execute()
        .await;

    // Phase 6: Inject facts into target aggregates
    //
    // Facts are events emitted by the saga that are injected directly into target
    // aggregates without command handling. The coordinator stamps sequence numbers
    // on receipt based on the aggregate's current state.
    //
    // Facts must have `external_id` set in their Cover for idempotent handling.
    // Fact injection failure fails the entire saga operation — facts are not
    // best-effort, they're part of the transaction.
    if let Some(fact_exec) = fact_executor {
        for fact in events {
            let domain = fact
                .cover
                .as_ref()
                .map(|c| c.domain.as_str())
                .unwrap_or("unknown");
            debug!(%domain, "Injecting fact from saga");

            fact_exec
                .inject(fact)
                .await
                .map_err(|e| BusError::SagaFailed {
                    name: saga_name.to_string(),
                    message: format!("Fact injection failed: {e}"),
                })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
