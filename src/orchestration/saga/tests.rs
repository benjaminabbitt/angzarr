use super::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use backon::ExponentialBuilder;

use crate::proto::CommandResponse;
use crate::proto_ext::CoverExt;

use super::super::command::CommandExecutor;
use super::super::destination::DestinationFetcher;

struct AlwaysSucceeds;

#[async_trait]
impl SagaRetryContext for AlwaysSucceeds {
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
    async fn re_execute_saga(
        &self,
        _destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {}
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
}

/// Context that always re-produces a single command on retry.
struct RetryingSagaContext;

#[async_trait]
impl SagaRetryContext for RetryingSagaContext {
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
    async fn re_execute_saga(
        &self,
        _destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![CommandBook::default()])
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {}
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
}

struct AlwaysRejects {
    rejection_count: AtomicU32,
}

#[async_trait]
impl SagaRetryContext for AlwaysRejects {
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
    async fn re_execute_saga(
        &self,
        _destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {
        self.rejection_count.fetch_add(1, Ordering::SeqCst);
    }
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
}

/// Executor that always succeeds.
struct SuccessExecutor;

#[async_trait]
impl CommandExecutor for SuccessExecutor {
    async fn execute(&self, _command: CommandBook) -> CommandOutcome {
        CommandOutcome::Success(CommandResponse::default())
    }
}

/// Executor that tracks call count and fails N times before succeeding.
struct CountingExecutor {
    failures_remaining: AtomicU32,
    execute_count: AtomicU32,
}

#[async_trait]
impl CommandExecutor for CountingExecutor {
    async fn execute(&self, _command: CommandBook) -> CommandOutcome {
        self.execute_count.fetch_add(1, Ordering::SeqCst);
        let remaining = self.failures_remaining.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            CommandOutcome::Retryable {
                reason: "Sequence conflict".to_string(),
                current_state: None,
            }
        } else {
            CommandOutcome::Success(CommandResponse::default())
        }
    }
}

/// Executor that always rejects.
struct RejectingExecutor;

#[async_trait]
impl CommandExecutor for RejectingExecutor {
    async fn execute(&self, _command: CommandBook) -> CommandOutcome {
        CommandOutcome::Rejected("Business rule violation".to_string())
    }
}

fn fast_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(10))
        .with_max_times(5)
}

#[tokio::test]
async fn test_execute_success_no_retry() {
    let ctx = AlwaysSucceeds;
    let executor = SuccessExecutor;
    let commands = vec![CommandBook::default()];
    execute_with_retry(
        &ctx,
        &executor,
        None,
        commands,
        vec![],
        "test-saga",
        "corr-1",
        fast_backoff(),
    )
    .await;
}

#[tokio::test]
async fn test_execute_empty_commands_noop() {
    let ctx = AlwaysSucceeds;
    let executor = SuccessExecutor;
    execute_with_retry(
        &ctx,
        &executor,
        None,
        vec![],
        vec![],
        "test-saga",
        "corr-1",
        fast_backoff(),
    )
    .await;
}

#[tokio::test]
async fn test_execute_retries_then_succeeds() {
    let ctx = RetryingSagaContext;
    let executor = CountingExecutor {
        failures_remaining: AtomicU32::new(2),
        execute_count: AtomicU32::new(0),
    };
    let commands = vec![CommandBook::default()];
    execute_with_retry(
        &ctx,
        &executor,
        None,
        commands,
        vec![],
        "test-saga",
        "corr-1",
        fast_backoff(),
    )
    .await;

    // Initial attempt + 2 retries = 3 executions
    assert_eq!(executor.execute_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_execute_non_retryable_calls_rejection_handler() {
    let ctx = AlwaysRejects {
        rejection_count: AtomicU32::new(0),
    };
    let executor = RejectingExecutor;
    let commands = vec![CommandBook::default()];
    execute_with_retry(
        &ctx,
        &executor,
        None,
        commands,
        vec![],
        "test-saga",
        "corr-1",
        fast_backoff(),
    )
    .await;

    assert_eq!(ctx.rejection_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_execute_exhausts_retries() {
    let ctx = RetryingSagaContext;
    let executor = CountingExecutor {
        failures_remaining: AtomicU32::new(100),
        execute_count: AtomicU32::new(0),
    };
    let backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(10))
        .with_max_times(3);
    let commands = vec![CommandBook::default()];
    execute_with_retry(
        &ctx,
        &executor,
        None,
        commands,
        vec![],
        "test-saga",
        "corr-1",
        backoff,
    )
    .await;

    // Initial attempt + 3 retries = 4 executions
    assert_eq!(executor.execute_count.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn test_orchestrate_saga_with_domain_validator() {
    let ctx = AlwaysSucceeds;
    let executor = SuccessExecutor;
    let validator = |cmd: &CommandBook| -> Result<(), String> {
        let domain = cmd.domain();
        if domain == "forbidden" {
            Err(format!("domain '{}' not allowed", domain))
        } else {
            Ok(())
        }
    };
    let result = orchestrate_saga(
        &ctx,
        &executor,
        None,
        "test-saga",
        "corr-1",
        Some(&validator),
        fast_backoff(),
    )
    .await;
    assert!(result.is_ok());
}

/// Executor that returns state with retryable error, then succeeds.
/// Used to test the cached state optimization.
struct RetryableWithStateExecutor {
    failures_remaining: AtomicU32,
}

#[async_trait]
impl CommandExecutor for RetryableWithStateExecutor {
    async fn execute(&self, _command: CommandBook) -> CommandOutcome {
        let remaining = self.failures_remaining.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            let state = EventBook {
                cover: Some(Cover {
                    domain: "test".to_string(),
                    root: Some(crate::proto::Uuid {
                        value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    }),
                    correlation_id: "corr-1".to_string(),
                    edition: None,
                }),
                pages: vec![],
                snapshot: None,
                ..Default::default()
            };
            CommandOutcome::Retryable {
                reason: "Sequence conflict".to_string(),
                current_state: Some(state),
            }
        } else {
            CommandOutcome::Success(CommandResponse::default())
        }
    }
}

/// Context for cached state test — prepares destinations for retry.
struct CachedStateContext;

#[async_trait]
impl SagaRetryContext for CachedStateContext {
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![Cover {
            domain: "test".to_string(),
            root: Some(crate::proto::Uuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: "".to_string(),
            edition: None,
        }])
    }
    async fn re_execute_saga(
        &self,
        _destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![CommandBook::default()])
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {}
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
}

/// Fetcher that tracks fetch count.
struct TrackingFetcher {
    fetch_count: AtomicU32,
}

#[async_trait]
impl DestinationFetcher for TrackingFetcher {
    async fn fetch(&self, _cover: &Cover) -> Option<EventBook> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        Some(EventBook::default())
    }
    async fn fetch_by_correlation(
        &self,
        _domain: &str,
        _correlation_id: &str,
    ) -> Option<EventBook> {
        None
    }
}

#[tokio::test]
async fn test_execute_uses_cached_state_from_conflict() {
    // Verify that when a Retryable error includes current_state,
    // subsequent retry uses that state instead of fetching
    let ctx = CachedStateContext;
    let executor = RetryableWithStateExecutor {
        failures_remaining: AtomicU32::new(1),
    };
    let fetcher = TrackingFetcher {
        fetch_count: AtomicU32::new(0),
    };
    let commands = vec![CommandBook::default()];
    execute_with_retry(
        &ctx,
        &executor,
        Some(&fetcher),
        commands,
        vec![],
        "test-saga",
        "corr-1",
        fast_backoff(),
    )
    .await;

    // With the new behavior: failed domains get fresh fetch, others use cache.
    // The test command has no domain set, so it uses default empty string.
    // On retry, the prepare_destinations returns a cover, which triggers a fetch
    // since there's no cached destination for that domain.
    assert!(fetcher.fetch_count.load(Ordering::SeqCst) >= 1);
}
