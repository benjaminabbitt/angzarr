use super::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use backon::ExponentialBuilder;

use crate::proto::CommandResponse;

/// PM context that always succeeds with no commands or PM events.
struct EmptyPm;

#[async_trait]
impl ProcessManagerContext for EmptyPm {
    async fn prepare(
        &self,
        _trigger: &EventBook,
        _pm_state: Option<&EventBook>,
    ) -> Result<PmPrepareResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(PmPrepareResponse { destinations: vec![] })
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
    ) -> Result<PmPrepareResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(PmPrepareResponse { destinations: vec![] })
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

fn fast_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(10))
        .with_max_times(5)
}

fn trigger_event() -> EventBook {
    use crate::proto::Cover;
    EventBook {
        cover: Some(Cover {
            domain: "order".to_string(),
            root: None,
            correlation_id: "corr-1".to_string(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
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
        fast_backoff(),
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
        fast_backoff(),
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
        fast_backoff(),
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

    let backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(10))
        .with_max_times(3);

    let result = orchestrate_pm(
        &ctx,
        &fetcher,
        &executor,
        &trigger,
        "fulfillment-pm",
        "corr-1",
        backoff,
    )
    .await;

    assert!(result.is_err());
    // Initial + 3 retries = 4 attempts, then exhausted
    assert_eq!(ctx.persist_attempts.load(Ordering::SeqCst), 4);
}
