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
use tracing::{debug, error, info, warn};

use crate::bus::BusError;
use crate::proto::{CommandBook, Cover, EventBook};
use crate::proto_ext::CoverExt;
use crate::utils::retry::RetryConfig;

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
}

/// Execute saga commands with retry on sequence conflicts.
///
/// On retryable errors: refreshes destination state via prepare + fetch,
/// re-invokes the saga, and retries with new commands.
/// On non-retryable errors: delegates to context for compensation.
///
/// When sequence conflicts include current aggregate state in the error response,
/// that state is cached and reused during retry to avoid redundant fetches.
pub async fn execute_with_retry(
    context: &dyn SagaRetryContext,
    executor: &dyn CommandExecutor,
    fetcher: Option<&dyn DestinationFetcher>,
    initial_commands: Vec<CommandBook>,
    correlation_id: &str,
    config: &RetryConfig,
) {
    if initial_commands.is_empty() {
        return;
    }

    let mut commands = initial_commands;
    let mut attempt = 0u32;

    loop {
        let mut needs_retry = false;
        // Cache states received from sequence conflict errors to avoid refetching
        let mut cached_states: HashMap<String, EventBook> = HashMap::new();

        for command in commands {
            // Ensure correlation_id is set on cover
            let mut command = command;
            if let Some(ref mut cover) = command.cover {
                if cover.correlation_id.is_empty() {
                    cover.correlation_id = correlation_id.to_string();
                }
            }

            let domain = command
                .cover
                .as_ref()
                .map(|c| c.domain.as_str())
                .unwrap_or("unknown");

            debug!(
                correlation_id = %correlation_id,
                domain = %domain,
                attempt = attempt,
                "Executing saga command"
            );

            match executor.execute(command.clone()).await {
                CommandOutcome::Success(_) => {
                    debug!(
                        correlation_id = %correlation_id,
                        domain = %domain,
                        "Saga command executed successfully"
                    );
                }
                CommandOutcome::Retryable {
                    reason,
                    current_state,
                } => {
                    warn!(
                        correlation_id = %correlation_id,
                        domain = %domain,
                        attempt = attempt,
                        error = %reason,
                        has_state = current_state.is_some(),
                        "Sequence conflict, will retry with fresh state"
                    );
                    needs_retry = true;

                    // Cache the state if provided to avoid refetching
                    if let Some(state) = current_state {
                        let key = state.cache_key();
                        cached_states.insert(key, state);
                    }
                }
                CommandOutcome::Rejected(reason) => {
                    error!(
                        correlation_id = %correlation_id,
                        domain = %domain,
                        error = %reason,
                        "Saga command rejected (non-retryable)"
                    );
                    context.on_command_rejected(&command, &reason).await;
                }
            }
        }

        if !needs_retry {
            break;
        }

        if !config.should_retry(attempt) {
            error!(
                correlation_id = %correlation_id,
                attempts = attempt + 1,
                "Saga retry exhausted"
            );
            break;
        }

        // Wait before retry
        let delay = config.delay_for_attempt(attempt);
        tokio::time::sleep(delay).await;
        attempt += 1;

        // Re-prepare: get fresh destination covers
        let covers = match context.prepare_destinations().await {
            Ok(c) => c,
            Err(e) => {
                error!(
                    correlation_id = %correlation_id,
                    error = %e,
                    "Saga re-prepare failed, aborting retry"
                );
                break;
            }
        };

        // Fetch state for destinations, using cached states when available
        let mut destinations = Vec::with_capacity(covers.len());
        let mut fetched_count = 0;
        let mut cached_count = 0;

        for cover in &covers {
            let cache_key = cover.cache_key();
            if let Some(cached) = cached_states.remove(&cache_key) {
                debug!(
                    correlation_id = %correlation_id,
                    domain = %cover.domain,
                    "Using cached state from conflict response"
                );
                destinations.push(cached);
                cached_count += 1;
            } else if let Some(f) = fetcher {
                if let Some(dest) = f.fetch(cover).await {
                    destinations.push(dest);
                    fetched_count += 1;
                }
            }
        }

        info!(
            correlation_id = %correlation_id,
            attempt = attempt,
            destinations = destinations.len(),
            fetched = fetched_count,
            cached = cached_count,
            "Retry prepared destination state"
        );

        // Re-execute saga with fresh state
        commands = match context.re_execute_saga(destinations).await {
            Ok(cmds) => {
                debug!(
                    correlation_id = %correlation_id,
                    attempt = attempt,
                    commands = cmds.len(),
                    "Saga retry produced new commands"
                );
                cmds
            }
            Err(e) => {
                error!(
                    correlation_id = %correlation_id,
                    attempt = attempt,
                    error = %e,
                    "Saga re-execute failed, aborting retry"
                );
                break;
            }
        };
    }
}

/// Full two-phase saga orchestration.
///
/// 1. Prepare: get destination covers from saga
/// 2. Fetch destination state
/// 3. Execute saga with source + destinations
/// 4. Validate output domains (if validator provided)
/// 5. Execute commands with retry
pub async fn orchestrate_saga(
    ctx: &dyn SagaRetryContext,
    executor: &dyn CommandExecutor,
    fetcher: Option<&dyn DestinationFetcher>,
    correlation_id: &str,
    output_domain_validator: Option<&OutputDomainValidator>,
    retry_config: &RetryConfig,
) -> Result<(), BusError> {
    // Phase 1: Prepare — get destination covers
    let destination_covers = ctx
        .prepare_destinations()
        .await
        .map_err(|e| BusError::Publish(e.to_string()))?;

    debug!(
        correlation_id = %correlation_id,
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
    let commands = ctx
        .re_execute_saga(destinations)
        .await
        .map_err(|e| BusError::Publish(e.to_string()))?;

    debug!(
        correlation_id = %correlation_id,
        commands = commands.len(),
        "Saga produced commands"
    );

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
        correlation_id,
        retry_config,
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests;
