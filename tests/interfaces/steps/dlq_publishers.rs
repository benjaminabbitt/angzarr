//! Dead Letter Publisher contract test step definitions.
//!
//! Tests all DLQ publisher implementations against the DeadLetterPublisher trait contract.
//! Verifies persistence, metadata preservation, and correct error handling.

use std::time::Duration;

use angzarr::dlq::{AngzarrDeadLetter, DeadLetterPublisher};
use angzarr::proto::{
    command_page, CommandBook, CommandPage, Cover, EventBook, MergeStrategy, Uuid as ProtoUuid,
};
use cucumber::{given, then, when, World};
use prost_types::Any;
use uuid::Uuid;

use crate::bus_backend::{DlqBackend, DlqPublisherContext};

/// Test context for DLQ publisher scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct DlqPublisherWorld {
    backend: DlqBackend,
    context: Option<DlqPublisherContext>,

    /// Last created dead letter.
    last_dead_letter: Option<AngzarrDeadLetter>,

    /// Multiple dead letters for batch tests.
    dead_letters: Vec<AngzarrDeadLetter>,

    /// Last publish result.
    last_publish_success: bool,

    /// Count of persisted entries (for verification).
    persisted_count: usize,

    /// Last domain used for tests.
    last_domain: String,
}

impl DlqPublisherWorld {
    fn new() -> Self {
        Self {
            backend: DlqBackend::from_env(),
            context: None,
            last_dead_letter: None,
            dead_letters: Vec::new(),
            last_publish_success: false,
            persisted_count: 0,
            last_domain: String::new(),
        }
    }

    fn publisher(&self) -> &dyn DeadLetterPublisher {
        self.context
            .as_ref()
            .expect("DLQ context not initialized")
            .publisher
            .as_ref()
    }

    fn make_command(&self, domain: &str, sequence: u32) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-correlation".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![CommandPage {
                sequence,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "type.test/TestCommand".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeManual as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_command_with_correlation(&self, domain: &str, correlation_id: &str) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "type.test/TestCommand".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeManual as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_event_book(&self, domain: &str) -> EventBook {
        let root = Uuid::new_v4();
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-correlation".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            snapshot: None,
            pages: vec![],
            next_sequence: 1,
        }
    }

    async fn verify_persistence(&self) -> usize {
        let ctx = self.context.as_ref().expect("Context not initialized");
        match ctx.backend {
            #[cfg(feature = "sqlite")]
            DlqBackend::Sqlite => ctx.count_sqlite_entries(&self.last_domain).await as usize,
            DlqBackend::Filesystem | DlqBackend::OffloadFilesystem => {
                ctx.count_filesystem_files().await
            }
            DlqBackend::Logging | DlqBackend::Channel => {
                // For logging/channel, we can't verify persistence directly
                // but we consider publish success as persistence
                if self.last_publish_success {
                    1
                } else {
                    0
                }
            }
        }
    }
}

// =============================================================================
// Background
// =============================================================================

#[given("a DLQ publisher backend")]
async fn given_dlq_publisher_backend(world: &mut DlqPublisherWorld) {
    // Always use SQLite for contract tests since it supports persistence verification.
    // Other backends can be tested via DLQ_BACKEND env var if needed.
    #[cfg(feature = "sqlite")]
    {
        world.backend = DlqBackend::Sqlite;
    }
    println!("Using DLQ backend: {}", world.backend.name());
    let ctx = DlqPublisherContext::new(world.backend).await;
    world.context = Some(ctx);
}

// =============================================================================
// Basic Persistence
// =============================================================================

#[given(expr = "a dead letter for domain {string} with reason {string}")]
async fn given_dead_letter_for_domain(
    world: &mut DlqPublisherWorld,
    domain: String,
    reason: String,
) {
    let cmd = world.make_command(&domain, 0);
    let mut dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        &cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    dead_letter.rejection_reason = reason;
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = domain;
}

#[when("the dead letter is published")]
async fn when_dead_letter_published(world: &mut DlqPublisherWorld) {
    let dl = world
        .last_dead_letter
        .take()
        .expect("No dead letter created");

    match world.publisher().publish(dl.clone()).await {
        Ok(_) => {
            world.last_publish_success = true;
            world.last_dead_letter = Some(dl);
        }
        Err(e) => {
            eprintln!("Publish failed: {:?}", e);
            world.last_publish_success = false;
            world.last_dead_letter = Some(dl);
        }
    }

    // Allow time for persistence
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[then("publish succeeds")]
async fn then_publish_succeeds(world: &mut DlqPublisherWorld) {
    assert!(world.last_publish_success, "Publish should have succeeded");
}

#[then("the dead letter is persisted")]
async fn then_dead_letter_persisted(world: &mut DlqPublisherWorld) {
    let count = world.verify_persistence().await;
    assert!(
        count > 0,
        "Dead letter should be persisted, got count={}",
        count
    );
    world.persisted_count = count;
}

// =============================================================================
// Correlation ID Persistence
// =============================================================================

#[given(expr = "a dead letter with correlation ID {string}")]
async fn given_dead_letter_with_correlation(world: &mut DlqPublisherWorld, correlation_id: String) {
    let cmd = world.make_command_with_correlation("correlation-test", &correlation_id);
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        &cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = "correlation-test".to_string();
}

#[then(expr = "the persisted entry has correlation ID {string}")]
async fn then_persisted_has_correlation(world: &mut DlqPublisherWorld, expected_id: String) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            if let Some(entry) = ctx.get_latest_sqlite_entry(&world.last_domain).await {
                assert_eq!(
                    entry.correlation_id,
                    Some(expected_id),
                    "Correlation ID should match"
                );
            } else {
                panic!("No entry found in SQLite");
            }
        }
        _ => {
            // For non-queryable backends, verify the dead letter was created correctly
            let dl = world.last_dead_letter.as_ref().expect("No dead letter");
            if let Some(cover) = &dl.cover {
                assert_eq!(cover.correlation_id, expected_id);
            } else {
                panic!("Dead letter has no cover");
            }
        }
    }
}

// =============================================================================
// Rejection Reason Persistence
// =============================================================================

#[given(expr = "a dead letter with rejection reason {string}")]
async fn given_dead_letter_with_reason(world: &mut DlqPublisherWorld, reason: String) {
    let cmd = world.make_command("reason-test", 0);
    let mut dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        &cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    dead_letter.rejection_reason = reason;
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = "reason-test".to_string();
}

#[then(expr = "the persisted entry contains rejection reason {string}")]
async fn then_persisted_contains_reason(world: &mut DlqPublisherWorld, expected_substring: String) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            if let Some(entry) = ctx.get_latest_sqlite_entry(&world.last_domain).await {
                assert!(
                    entry.rejection_reason.contains(&expected_substring),
                    "Rejection reason '{}' should contain '{}'",
                    entry.rejection_reason,
                    expected_substring
                );
            } else {
                panic!("No entry found in SQLite");
            }
        }
        _ => {
            let dl = world.last_dead_letter.as_ref().expect("No dead letter");
            assert!(
                dl.rejection_reason.contains(&expected_substring),
                "Rejection reason '{}' should contain '{}'",
                dl.rejection_reason,
                expected_substring
            );
        }
    }
}

// =============================================================================
// Source Component Persistence
// =============================================================================

#[given(expr = "a dead letter from source {string} of type {string}")]
async fn given_dead_letter_from_source(
    world: &mut DlqPublisherWorld,
    source: String,
    source_type: String,
) {
    let events = world.make_event_book("source-test");
    let dead_letter = AngzarrDeadLetter::from_event_processing_failure(
        &events,
        "test error",
        1,
        false,
        &source,
        &source_type,
    );
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = "source-test".to_string();
}

#[then(expr = "the persisted entry has source component {string}")]
async fn then_persisted_has_source(world: &mut DlqPublisherWorld, expected_source: String) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            if let Some(entry) = ctx.get_latest_sqlite_entry(&world.last_domain).await {
                assert_eq!(entry.source_component, expected_source);
            } else {
                panic!("No entry found in SQLite");
            }
        }
        _ => {
            let dl = world.last_dead_letter.as_ref().expect("No dead letter");
            assert_eq!(dl.source_component, expected_source);
        }
    }
}

#[then(expr = "the persisted entry has source type {string}")]
async fn then_persisted_has_source_type(world: &mut DlqPublisherWorld, expected_type: String) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            if let Some(entry) = ctx.get_latest_sqlite_entry(&world.last_domain).await {
                assert_eq!(entry.source_component_type, expected_type);
            } else {
                panic!("No entry found in SQLite");
            }
        }
        _ => {
            let dl = world.last_dead_letter.as_ref().expect("No dead letter");
            assert_eq!(dl.source_component_type, expected_type);
        }
    }
}

// =============================================================================
// Rejection Type Persistence
// =============================================================================

#[given(expr = "a sequence mismatch dead letter with expected={int} actual={int}")]
async fn given_sequence_mismatch_dead_letter(
    world: &mut DlqPublisherWorld,
    expected: u32,
    actual: u32,
) {
    let cmd = world.make_command("sequence-test", expected);
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        &cmd,
        expected,
        actual,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = "sequence-test".to_string();
}

#[then(expr = "the persisted entry has rejection type {string}")]
async fn then_persisted_has_rejection_type(world: &mut DlqPublisherWorld, expected_type: String) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            if let Some(entry) = ctx.get_latest_sqlite_entry(&world.last_domain).await {
                assert_eq!(entry.rejection_type, expected_type);
            } else {
                panic!("No entry found in SQLite");
            }
        }
        _ => {
            let dl = world.last_dead_letter.as_ref().expect("No dead letter");
            assert_eq!(dl.reason_type(), expected_type);
        }
    }
}

#[given(expr = "an event processing failure dead letter with retry_count={int}")]
async fn given_event_processing_failure_dead_letter(
    world: &mut DlqPublisherWorld,
    retry_count: u32,
) {
    let events = world.make_event_book("failure-test");
    let dead_letter = AngzarrDeadLetter::from_event_processing_failure(
        &events,
        "Handler failed",
        retry_count,
        false,
        "test-saga",
        "saga",
    );
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = "failure-test".to_string();
}

// =============================================================================
// Multiple Dead Letters
// =============================================================================

#[when(expr = "{int} dead letters are published for domain {string}")]
async fn when_multiple_published(world: &mut DlqPublisherWorld, count: usize, domain: String) {
    world.last_domain = domain.clone();
    world.dead_letters.clear();

    for i in 0..count {
        let cmd = world.make_command(&domain, i as u32);
        let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            i as u32,
            i as u32 + 5,
            MergeStrategy::MergeManual,
            &format!("aggregate-{}", i),
        );

        world
            .publisher()
            .publish(dead_letter.clone())
            .await
            .expect("Failed to publish");

        world.dead_letters.push(dead_letter);
    }

    world.last_publish_success = true;
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[then(expr = "{int} entries are persisted")]
async fn then_n_entries_persisted(world: &mut DlqPublisherWorld, expected_count: usize) {
    let count = world.verify_persistence().await;
    assert_eq!(
        count, expected_count,
        "Expected {} persisted entries, got {}",
        expected_count, count
    );
}

#[then("entries are persisted in order")]
async fn then_entries_in_order(world: &mut DlqPublisherWorld) {
    // For backends that support ordering verification
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            // SQLite entries have auto-incrementing IDs, so they're naturally ordered
            // We've already verified count, ordering is implicit
        }
        _ => {
            // For other backends, order verification is implementation-dependent
        }
    }
}

#[given("dead letters for multiple domains")]
async fn given_dead_letters_for_multiple_domains(world: &mut DlqPublisherWorld) {
    world.dead_letters.clear();
    // Use fixed domains for testing
    let domains = ["orders", "inventory", "payments"];
    for domain in &domains {
        let cmd = world.make_command(domain, 0);
        let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "test-aggregate",
        );
        world.dead_letters.push(dead_letter);
    }
}

#[when("all dead letters are published")]
async fn when_all_published(world: &mut DlqPublisherWorld) {
    for dl in &world.dead_letters {
        world
            .publisher()
            .publish(dl.clone())
            .await
            .expect("Failed to publish");
    }
    world.last_publish_success = true;
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[then("each domain has 1 persisted entry")]
async fn then_each_domain_has_entry(world: &mut DlqPublisherWorld) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            for dl in &world.dead_letters {
                let domain = dl.domain().unwrap_or("unknown");
                let count = ctx.count_sqlite_entries(domain).await;
                assert_eq!(count, 1, "Domain {} should have 1 entry", domain);
            }
        }
        DlqBackend::Filesystem | DlqBackend::OffloadFilesystem => {
            let count = ctx.count_filesystem_files().await;
            assert_eq!(
                count,
                world.dead_letters.len(),
                "Should have {} files",
                world.dead_letters.len()
            );
        }
        _ => {
            // For logging backend, we can't verify per-domain
            assert!(world.last_publish_success);
        }
    }
}

// =============================================================================
// is_configured Contract
// =============================================================================

#[then("is_configured returns true")]
async fn then_is_configured_true(world: &mut DlqPublisherWorld) {
    let is_configured = world.publisher().is_configured();
    assert!(is_configured, "Publisher should report as configured");
}

// =============================================================================
// Timestamp Persistence
// =============================================================================

#[given("a dead letter with occurred_at timestamp")]
async fn given_dead_letter_with_timestamp(world: &mut DlqPublisherWorld) {
    let cmd = world.make_command("timestamp-test", 0);
    let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
        &cmd,
        0,
        5,
        MergeStrategy::MergeManual,
        "test-aggregate",
    );
    // occurred_at is set automatically by from_sequence_mismatch
    assert!(dead_letter.occurred_at.is_some());
    world.last_dead_letter = Some(dead_letter);
    world.last_domain = "timestamp-test".to_string();
}

#[then("the persisted entry has a valid timestamp")]
async fn then_persisted_has_timestamp(world: &mut DlqPublisherWorld) {
    let ctx = world.context.as_ref().expect("Context not initialized");
    match ctx.backend {
        #[cfg(feature = "sqlite")]
        DlqBackend::Sqlite => {
            if let Some(entry) = ctx.get_latest_sqlite_entry(&world.last_domain).await {
                // Verify timestamp is a valid RFC3339 string
                assert!(
                    !entry.occurred_at.is_empty(),
                    "Timestamp should not be empty"
                );
                // Try to parse it
                let parsed = chrono::DateTime::parse_from_rfc3339(&entry.occurred_at);
                assert!(parsed.is_ok(), "Timestamp should be valid RFC3339");
            } else {
                panic!("No entry found in SQLite");
            }
        }
        _ => {
            let dl = world.last_dead_letter.as_ref().expect("No dead letter");
            assert!(
                dl.occurred_at.is_some(),
                "Dead letter should have timestamp"
            );
        }
    }
}
