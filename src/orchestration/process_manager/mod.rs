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
use tracing::{debug, error, info, warn};

use crate::bus::BusError;
use crate::proto::{CommandBook, Cover, EventBook};
use crate::utils::retry::RetryConfig;

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
pub async fn orchestrate_pm(
    ctx: &dyn ProcessManagerContext,
    fetcher: &dyn DestinationFetcher,
    executor: &dyn CommandExecutor,
    trigger: &EventBook,
    pm_domain: &str,
    correlation_id: &str,
    retry_config: &RetryConfig,
) -> Result<(), BusError> {
    let trigger_domain = trigger
        .cover
        .as_ref()
        .map(|c| c.domain.as_str())
        .unwrap_or("unknown");

    debug!(
        correlation_id = %correlation_id,
        trigger_domain = %trigger_domain,
        process_domain = %pm_domain,
        "Processing event in process manager"
    );

    let mut attempt = 0u32;
    loop {
        // Load trigger domain state by correlation_id
        let trigger_state = fetcher
            .fetch_by_correlation(trigger_domain, correlation_id)
            .await
            .unwrap_or_else(|| {
                warn!(
                    correlation_id = %correlation_id,
                    domain = %trigger_domain,
                    "Failed to fetch trigger state, using incoming event"
                );
                trigger.clone()
            });

        // Load PM state by correlation_id
        let pm_state = fetcher
            .fetch_by_correlation(pm_domain, correlation_id)
            .await;

        if pm_state.is_none() {
            debug!(
                correlation_id = %correlation_id,
                domain = %pm_domain,
                "No existing PM state (new workflow)"
            );
        }

        // Phase 1: Prepare — get additional destination covers
        let destination_covers = ctx
            .prepare(&trigger_state, pm_state.as_ref())
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;

        debug!(
            correlation_id = %correlation_id,
            destinations = destination_covers.len(),
            "ProcessManager.Prepare returned destinations"
        );

        // Fetch additional destinations
        let mut destinations = Vec::with_capacity(destination_covers.len());
        for cover in &destination_covers {
            if let Some(event_book) = fetcher.fetch(cover).await {
                destinations.push(event_book);
            } else {
                warn!(
                    correlation_id = %correlation_id,
                    domain = %cover.domain,
                    "Failed to fetch destination, skipping"
                );
            }
        }

        // Phase 2: Handle — produce commands + PM events
        let response = ctx
            .handle(&trigger_state, pm_state.as_ref(), &destinations)
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;

        debug!(
            correlation_id = %correlation_id,
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
                            correlation_id = %correlation_id,
                            domain = %pm_domain,
                            events = process_events.pages.len(),
                            "PM events persisted successfully"
                        );
                    }
                    CommandOutcome::Retryable { reason, .. }
                        if attempt < retry_config.max_retries =>
                    {
                        warn!(
                            correlation_id = %correlation_id,
                            attempt = attempt,
                            error = %reason,
                            "Sequence conflict persisting PM events, retrying"
                        );
                        let delay = retry_config.delay_for_attempt(attempt);
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    CommandOutcome::Retryable { reason, .. }
                    | CommandOutcome::Rejected(reason) => {
                        error!(
                            correlation_id = %correlation_id,
                            domain = %pm_domain,
                            error = %reason,
                            "Failed to persist PM events"
                        );
                        return Err(BusError::Publish(reason));
                    }
                }
            }
        }

        // Execute commands produced by process manager
        for mut command_book in response.commands {
            if let Some(ref mut cover) = command_book.cover {
                if cover.correlation_id.is_empty() {
                    cover.correlation_id = correlation_id.to_string();
                }
            }

            let cmd_domain = command_book
                .cover
                .as_ref()
                .map(|c| c.domain.clone())
                .unwrap_or_else(|| "unknown".to_string());

            debug!(
                correlation_id = %correlation_id,
                domain = %cmd_domain,
                "Executing process manager command"
            );

            match executor.execute(command_book).await {
                CommandOutcome::Success(cmd_response) => {
                    debug!(
                        correlation_id = %correlation_id,
                        domain = %cmd_domain,
                        has_events = cmd_response.events.is_some(),
                        "Process manager command executed successfully"
                    );
                }
                CommandOutcome::Retryable { reason, .. } => {
                    warn!(
                        correlation_id = %correlation_id,
                        domain = %cmd_domain,
                        error = %reason,
                        "Command sequence conflict, will retry on next trigger"
                    );
                }
                CommandOutcome::Rejected(reason) => {
                    error!(
                        correlation_id = %correlation_id,
                        domain = %cmd_domain,
                        error = %reason,
                        "Process manager command failed (non-retryable)"
                    );
                }
            }
        }

        // Success — exit retry loop
        break;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use crate::proto::CommandResponse;

    /// PM context that always succeeds with no commands or PM events.
    struct EmptyPm;

    #[async_trait]
    impl ProcessManagerContext for EmptyPm {
        async fn prepare(
            &self,
            _trigger: &EventBook,
            _pm_state: Option<&EventBook>,
        ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(vec![])
        }
        async fn handle(
            &self,
            _trigger: &EventBook,
            _pm_state: Option<&EventBook>,
            _destinations: &[EventBook],
        ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
            Ok(PmHandleResponse {
                commands: vec![],
                process_events: None,
            })
        }
        async fn persist_pm_events(
            &self,
            _process_events: &EventBook,
            _correlation_id: &str,
        ) -> CommandOutcome {
            CommandOutcome::Success(CommandResponse::default())
        }
    }

    /// PM context that returns PM events that need persisting.
    struct PmWithEvents {
        persist_attempts: AtomicU32,
        fail_persist_times: u32,
    }

    #[async_trait]
    impl ProcessManagerContext for PmWithEvents {
        async fn prepare(
            &self,
            _trigger: &EventBook,
            _pm_state: Option<&EventBook>,
        ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(vec![])
        }
        async fn handle(
            &self,
            _trigger: &EventBook,
            _pm_state: Option<&EventBook>,
            _destinations: &[EventBook],
        ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
            use crate::proto::EventPage;
            Ok(PmHandleResponse {
                commands: vec![],
                process_events: Some(EventBook {
                    cover: None,
                    pages: vec![EventPage::default()],
                    snapshot: None,
                    snapshot_state: None,
                }),
            })
        }
        async fn persist_pm_events(
            &self,
            _process_events: &EventBook,
            _correlation_id: &str,
        ) -> CommandOutcome {
            let attempt = self.persist_attempts.fetch_add(1, Ordering::SeqCst);
            if attempt < self.fail_persist_times {
                CommandOutcome::Retryable {
                    reason: "Sequence conflict".to_string(),
                    current_state: None,
                }
            } else {
                CommandOutcome::Success(CommandResponse::default())
            }
        }
    }

    /// Destination fetcher that always returns None.
    struct NoOpFetcher;

    #[async_trait]
    impl DestinationFetcher for NoOpFetcher {
        async fn fetch(&self, _cover: &Cover) -> Option<EventBook> {
            None
        }
        async fn fetch_by_correlation(
            &self,
            _domain: &str,
            _correlation_id: &str,
        ) -> Option<EventBook> {
            None
        }
    }

    /// Command executor that always succeeds.
    struct NoOpExecutor;

    #[async_trait]
    impl CommandExecutor for NoOpExecutor {
        async fn execute(&self, _command: CommandBook) -> CommandOutcome {
            CommandOutcome::Success(CommandResponse::default())
        }
    }

    fn fast_retry_config() -> RetryConfig {
        RetryConfig {
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            max_retries: 5,
            jitter: 0.0,
        }
    }

    fn trigger_event() -> EventBook {
        use crate::proto::Cover;
        EventBook {
            cover: Some(Cover {
                domain: "order".to_string(),
                root: None,
                correlation_id: "corr-1".to_string(),
            }),
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_orchestrate_pm_empty_response() {
        let ctx = EmptyPm;
        let fetcher = NoOpFetcher;
        let executor = NoOpExecutor;
        let trigger = trigger_event();

        let result = orchestrate_pm(
            &ctx,
            &fetcher,
            &executor,
            &trigger,
            "fulfillment-pm",
            "corr-1",
            &fast_retry_config(),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrate_pm_persists_events() {
        let ctx = PmWithEvents {
            persist_attempts: AtomicU32::new(0),
            fail_persist_times: 0,
        };
        let fetcher = NoOpFetcher;
        let executor = NoOpExecutor;
        let trigger = trigger_event();

        let result = orchestrate_pm(
            &ctx,
            &fetcher,
            &executor,
            &trigger,
            "fulfillment-pm",
            "corr-1",
            &fast_retry_config(),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(ctx.persist_attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_orchestrate_pm_retries_on_sequence_conflict() {
        let ctx = PmWithEvents {
            persist_attempts: AtomicU32::new(0),
            fail_persist_times: 2,
        };
        let fetcher = NoOpFetcher;
        let executor = NoOpExecutor;
        let trigger = trigger_event();

        let result = orchestrate_pm(
            &ctx,
            &fetcher,
            &executor,
            &trigger,
            "fulfillment-pm",
            "corr-1",
            &fast_retry_config(),
        )
        .await;

        assert!(result.is_ok());
        // 2 failed + 1 success = 3 attempts
        assert_eq!(ctx.persist_attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_orchestrate_pm_exhausts_retries() {
        let ctx = PmWithEvents {
            persist_attempts: AtomicU32::new(0),
            fail_persist_times: 100,
        };
        let fetcher = NoOpFetcher;
        let executor = NoOpExecutor;
        let trigger = trigger_event();

        let config = RetryConfig {
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            max_retries: 3,
            jitter: 0.0,
        };

        let result = orchestrate_pm(
            &ctx,
            &fetcher,
            &executor,
            &trigger,
            "fulfillment-pm",
            "corr-1",
            &config,
        )
        .await;

        assert!(result.is_err());
        // Initial + 3 retries = 4 attempts, then exhausted
        assert_eq!(ctx.persist_attempts.load(Ordering::SeqCst), 4);
    }
}
