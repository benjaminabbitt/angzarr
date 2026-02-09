//! Saga orchestration abstraction.
//!
//! `SagaRetryContext` defines operations for saga retry loops.
//! `execute_with_retry` implements the retry-with-backoff protocol.
//! `orchestrate_saga` implements the full two-phase saga flow.
//!
//! - `local/`: in-process saga handler calls
//! - `grpc/`: remote gRPC saga client calls

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use backon::ExponentialBuilder;
use tracing::{debug, error, warn};

use crate::bus::BusError;
use crate::proto::{CommandBook, Cover, EventBook, SagaCommandOrigin};
use crate::proto_ext::CoverExt;
use crate::utils::retry::{run_with_retry, RetryOutcome, RetryableOperation};

use super::command::{CommandExecutor, CommandOutcome};
use super::destination::DestinationFetcher;

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

/// Operations needed by the saga retry loop.
///
/// Each transport mode implements this trait to provide saga-specific
/// invocation (prepare, execute, compensation). Command execution and
/// destination fetching are passed separately to `orchestrate_saga`
/// and `execute_with_retry`, matching the PM pattern.
///
/// One instance per saga invocation — captures the per-invocation context
/// (source event book, saga handler, etc.)
#[async_trait]
pub trait SagaRetryContext: Send + Sync {
    /// Re-invoke the saga's prepare phase to get destination covers.
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>>;

    /// Re-invoke the saga's execute phase with fresh destination state.
    /// Returns new commands to execute.
    async fn re_execute_saga(
        &self,
        destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>>;

    /// Handle a permanently rejected command (compensation, logging, etc.)
    async fn on_command_rejected(&self, command: &CommandBook, reason: &str);

    /// Cover of the source event that triggered this saga invocation.
    ///
    /// Used to populate `saga_origin` on outgoing commands, enabling
    /// the aggregate to skip sequence validation and supporting
    /// compensation flow on rejection.
    fn source_cover(&self) -> Option<&Cover>;
}

/// State for a retryable saga command execution operation.
struct SagaOperation<'a> {
    context: &'a dyn SagaRetryContext,
    executor: &'a dyn CommandExecutor,
    fetcher: Option<&'a dyn DestinationFetcher>,
    correlation_id: &'a str,
    commands: Vec<CommandBook>,
    cached_states: HashMap<String, EventBook>,
}

#[async_trait]
impl<'a> RetryableOperation for SagaOperation<'a> {
    type Success = ();
    type Failure = String;

    fn name(&self) -> &str {
        "saga_command_execution"
    }

    async fn try_execute(&mut self) -> RetryOutcome<Self::Success, Self::Failure> {
        let mut needs_retry = false;
        self.cached_states.clear();

        for command in &self.commands {
            let mut command = command.clone();
            if let Some(ref mut cover) = command.cover {
                if cover.correlation_id.is_empty() {
                    cover.correlation_id = self.correlation_id.to_string();
                }
            }

            let domain = command.domain();

            match self.executor.execute(command.clone()).await {
                CommandOutcome::Success(_) => {
                    debug!(%domain, "Saga command executed successfully");
                }
                CommandOutcome::Retryable {
                    reason,
                    current_state,
                } => {
                    warn!(%domain, error = %reason, "Sequence conflict, will retry");
                    needs_retry = true;
                    if let Some(state) = current_state {
                        self.cached_states.insert(state.cache_key(), state);
                    }
                }
                CommandOutcome::Rejected(reason) => {
                    error!(%domain, error = %reason, "Saga command rejected (non-retryable)");
                    self.context.on_command_rejected(&command, &reason).await;
                }
            }
        }

        if needs_retry {
            RetryOutcome::Retryable("Sequence conflict".to_string())
        } else {
            RetryOutcome::Success(())
        }
    }

    async fn prepare_for_retry(&mut self, _failure: &Self::Failure) -> Result<(), Self::Failure> {
        // Re-prepare: get fresh destination covers
        let covers = self
            .context
            .prepare_destinations()
            .await
            .map_err(|e| e.to_string())?;

        // Fetch state for destinations
        let mut destinations = Vec::with_capacity(covers.len());
        for cover in &covers {
            if let Some(cached) = self.cached_states.remove(&cover.cache_key()) {
                destinations.push(cached);
            } else if let Some(f) = self.fetcher {
                if let Some(dest) = f.fetch(cover).await {
                    destinations.push(dest);
                }
            }
        }

        // Re-execute saga with fresh state
        self.commands = self
            .context
            .re_execute_saga(destinations)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

/// Execute saga commands with retry on sequence conflicts.
#[tracing::instrument(name = "saga.retry", skip_all, fields(%saga_name, %correlation_id))]
async fn execute_with_retry(
    context: &dyn SagaRetryContext,
    executor: &dyn CommandExecutor,
    fetcher: Option<&dyn DestinationFetcher>,
    initial_commands: Vec<CommandBook>,
    saga_name: &str,
    correlation_id: &str,
    backoff: ExponentialBuilder,
) {
    if initial_commands.is_empty() {
        return;
    }

    let operation = SagaOperation {
        context,
        executor,
        fetcher,
        correlation_id,
        commands: initial_commands,
        cached_states: HashMap::new(),
    };

    if let Err(e) = run_with_retry(operation, backoff).await {
        error!(error = %e, "Saga execution failed after multiple retries");
    }
}

/// Full two-phase saga orchestration.
///
/// 1. Prepare: get destination covers from saga
/// 2. Fetch destination state
/// 3. Execute saga with source + destinations
/// 4. Validate output domains (if validator provided)
/// 5. Execute commands with retry
#[tracing::instrument(name = "saga.orchestrate", skip_all, fields(%saga_name, %correlation_id))]
pub async fn orchestrate_saga(
    ctx: &dyn SagaRetryContext,
    executor: &dyn CommandExecutor,
    fetcher: Option<&dyn DestinationFetcher>,
    saga_name: &str,
    correlation_id: &str,
    output_domain_validator: Option<&OutputDomainValidator>,
    backoff: ExponentialBuilder,
) -> Result<(), BusError> {
    #[cfg(feature = "otel")]
    let start = std::time::Instant::now();

    // Phase 1: Prepare — get destination covers
    let destination_covers = ctx
        .prepare_destinations()
        .await
        .map_err(|e| BusError::Publish(e.to_string()))?;

    debug!(
        destinations = destination_covers.len(),
        "Saga prepare returned destinations"
    );

    // Phase 2: Fetch destination EventBooks
    let destinations = if let Some(f) = fetcher {
        super::shared::fetch_destinations(f, &destination_covers, correlation_id).await
    } else {
        vec![]
    };

    // Phase 3: Execute saga with source + destinations
    let mut commands = ctx
        .re_execute_saga(destinations)
        .await
        .map_err(|e| BusError::Publish(e.to_string()))?;

    // Stamp saga_origin on all commands so the aggregate can:
    // (a) skip sequence validation (sagas don't track target sequences)
    // (b) initiate compensation flow on rejection
    let source_cover = ctx.source_cover().cloned();
    for cmd in &mut commands {
        if cmd.saga_origin.is_none() {
            cmd.saga_origin = Some(SagaCommandOrigin {
                saga_name: saga_name.to_string(),
                triggering_aggregate: source_cover.clone(),
                triggering_event_sequence: 0,
            });
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

    // Phase 5: Execute commands with retry
    execute_with_retry(
        ctx,
        executor,
        fetcher,
        commands,
        saga_name,
        correlation_id,
        backoff,
    )
    .await;

    #[cfg(feature = "otel")]
    {
        use crate::utils::metrics::{self, SAGA_DURATION};
        SAGA_DURATION.record(
            start.elapsed().as_secs_f64(),
            &[
                metrics::component_attr("saga"),
                metrics::name_attr(saga_name),
            ],
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests;
