//! Integration test: PM persist -> real SqliteEventStore.
//!
//! Run with:
//! ```bash
//! cargo test --test pm_persist_event_store --features "test-utils" -- --nocapture
//! ```
//!
//! Sister test to `aggregate_pipeline_event_store.rs`. The
//! `GrpcPMContext::persist_pm_events` path at
//! `process_manager/grpc/mod.rs:115` writes PM-state events directly
//! to the event store, bypassing the aggregate command pipeline.
//! `PmWithEvents` in `process_manager/tests.rs` stubs the outcome,
//! so unit tests never see this code run against a real store. The
//! storage tests in `storage_sqlite.rs` exercise `EventStore.add`
//! at the trait surface but don't know anything about PM's
//! edition / correlation-id / pm_root extraction logic.
//!
//! This test wires `persist_pm_event_book` (the free fn extracted
//! from `GrpcPMContext::persist_pm_events` for testability) to a
//! real in-memory SQLite event store + a tracking event bus, and
//! verifies:
//!
//! - PM root from the event book's cover is extracted and used as
//!   the storage key.
//! - PM events are persisted under the PM's `pm_domain` argument
//!   (not whatever the trigger's domain was).
//! - Edition propagates from the book's cover to the store column.
//! - After persist, the bus sees the re-read book published.
//! - Two consecutive persist calls produce events at sequences 0, 1.
//! - A sequence conflict (re-using a sequence the store already has)
//!   returns `CommandOutcome::Rejected { code: Internal, .. }`,
//!   which `orchestrate_pm` classifies as immediate-Rejected per
//!   R2-15 (does NOT count toward retry budget).

#![cfg(feature = "test-utils")]

use std::sync::Arc;

use prost_types::Any;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::Mutex;
use uuid::Uuid;

use angzarr::bus::{self, EventBus, EventHandler, PublishResult};
use angzarr::orchestration::command::CommandOutcome;
use angzarr::orchestration::process_manager::grpc::persist_pm_event_book;
use angzarr::proto::{
    event_page, page_header, Cover, Edition, EventBook, EventPage, PageHeader, Uuid as ProtoUuid,
};
use angzarr::storage::{EventStore, SqliteEventStore};
use async_trait::async_trait;

// ============================================================================
// Fixtures
// ============================================================================

async fn create_sqlite_event_store() -> Arc<SqliteEventStore> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("connect SQLite pool");
    sqlx::migrate!("./migrations/sqlite")
        .run(&pool)
        .await
        .expect("run sqlite migrations");
    Arc::new(SqliteEventStore::new(pool))
}

/// Event bus that records every publish so the test can assert which
/// books reached the bus alongside the storage write.
struct RecordingEventBus {
    published: Mutex<Vec<EventBook>>,
}

impl RecordingEventBus {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            published: Mutex::new(Vec::new()),
        })
    }

    async fn calls(&self) -> Vec<EventBook> {
        self.published.lock().await.clone()
    }
}

#[async_trait]
impl EventBus for RecordingEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> bus::error::Result<PublishResult> {
        self.published.lock().await.push((*book).clone());
        Ok(PublishResult::default())
    }

    async fn subscribe(&self, _handler: Box<dyn EventHandler>) -> bus::error::Result<()> {
        unimplemented!("Not needed for these tests")
    }

    async fn create_subscriber(
        &self,
        _name: &str,
        _domain_filter: Option<&str>,
    ) -> bus::error::Result<Arc<dyn EventBus>> {
        unimplemented!("Not needed for these tests")
    }
}

fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

fn pm_event_book(
    pm_domain: &str,
    pm_root: Uuid,
    correlation_id: &str,
    edition: Option<&str>,
    sequences: &[u32],
) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: pm_domain.to_string(),
            root: Some(proto_uuid(pm_root)),
            correlation_id: correlation_id.to_string(),
            edition: edition.map(|name| Edition {
                name: name.to_string(),
                divergences: vec![],
            }),
            ext: None,
        }),
        pages: sequences
            .iter()
            .map(|&seq| EventPage {
                header: Some(PageHeader {
                    sync_mode: None,
                    sequence_type: Some(page_header::SequenceType::Sequence(seq)),
                }),
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "test.PmEvent".to_string(),
                    value: vec![],
                })),
                created_at: None,
                ..Default::default()
            })
            .collect(),
        snapshot: None,
        ..Default::default()
    }
}

fn event_sequence_num(page: &EventPage) -> u32 {
    match page.header.as_ref().and_then(|h| h.sequence_type.as_ref()) {
        Some(page_header::SequenceType::Sequence(s)) => *s,
        other => panic!("expected Sequence variant, got {other:?}"),
    }
}

// ============================================================================
// Tests
// ============================================================================

/// A PM event book persists to the SQLite store under the PM's
/// `pm_domain` + the cover's root + default edition. The bus sees
/// the re-read book published.
#[tokio::test]
async fn pm_persist_writes_event_book_to_store_and_bus() {
    let event_store = create_sqlite_event_store().await;
    let bus_recorder = RecordingEventBus::new();
    let event_bus: Arc<dyn EventBus> = bus_recorder.clone();
    let pm_root = Uuid::new_v4();

    let book = pm_event_book("fulfillment-pm", pm_root, "corr-1", None, &[0]);
    let outcome = persist_pm_event_book(
        &(event_store.clone() as Arc<dyn EventStore>),
        &event_bus,
        "fulfillment-pm",
        &book,
        "corr-1",
    )
    .await;
    assert!(
        matches!(outcome, CommandOutcome::Success(_)),
        "persist outcome must be Success, got {outcome:?}"
    );

    // Stored under (pm_domain, default_edition, pm_root).
    let persisted = event_store
        .get("fulfillment-pm", "", pm_root)
        .await
        .expect("event_store.get");
    assert_eq!(persisted.len(), 1, "expected 1 persisted PM event");
    assert_eq!(event_sequence_num(&persisted[0]), 0);

    // The bus saw exactly one publish containing the re-read book.
    let bus_calls = bus_recorder.calls().await;
    assert_eq!(bus_calls.len(), 1, "expected exactly one bus publish");
    assert_eq!(
        bus_calls[0].pages.len(),
        1,
        "published book must carry the re-read event"
    );
}

/// Two consecutive persist calls on the same PM root produce events
/// at sequences 0, 1. Pins the PM-persist re-read flow against a
/// real store, mirroring the aggregate `pipeline_increments_sequence`
/// test.
#[tokio::test]
async fn pm_persist_increments_sequence_across_two_calls() {
    let event_store = create_sqlite_event_store().await;
    let bus_recorder = RecordingEventBus::new();
    let event_bus: Arc<dyn EventBus> = bus_recorder.clone();
    let pm_root = Uuid::new_v4();

    let book0 = pm_event_book("pm-domain", pm_root, "corr-1", None, &[0]);
    let outcome0 = persist_pm_event_book(
        &(event_store.clone() as Arc<dyn EventStore>),
        &event_bus,
        "pm-domain",
        &book0,
        "corr-1",
    )
    .await;
    assert!(matches!(outcome0, CommandOutcome::Success(_)));

    let book1 = pm_event_book("pm-domain", pm_root, "corr-1", None, &[1]);
    let outcome1 = persist_pm_event_book(
        &(event_store.clone() as Arc<dyn EventStore>),
        &event_bus,
        "pm-domain",
        &book1,
        "corr-1",
    )
    .await;
    assert!(matches!(outcome1, CommandOutcome::Success(_)));

    let persisted = event_store
        .get("pm-domain", "", pm_root)
        .await
        .expect("event_store.get");
    assert_eq!(persisted.len(), 2);
    assert_eq!(event_sequence_num(&persisted[0]), 0);
    assert_eq!(event_sequence_num(&persisted[1]), 1);

    // Two bus publishes -- one per persist.
    let bus_calls = bus_recorder.calls().await;
    assert_eq!(bus_calls.len(), 2);
}

/// PM events with `cover.edition = "branch-x"` persist under that
/// edition, not the default. The default-edition view sees zero
/// events. Mirrors the aggregate edition-propagation test against
/// PM's persist path (which extracts edition from the cover via
/// `process_events.edition()`).
#[tokio::test]
async fn pm_persist_propagates_edition_to_store() {
    let event_store = create_sqlite_event_store().await;
    let event_bus: Arc<dyn EventBus> = RecordingEventBus::new();
    let pm_root = Uuid::new_v4();

    let book = pm_event_book("pm-domain", pm_root, "corr-1", Some("branch-x"), &[0]);
    let outcome = persist_pm_event_book(
        &(event_store.clone() as Arc<dyn EventStore>),
        &event_bus,
        "pm-domain",
        &book,
        "corr-1",
    )
    .await;
    assert!(matches!(outcome, CommandOutcome::Success(_)));

    let branch = event_store
        .get("pm-domain", "branch-x", pm_root)
        .await
        .expect("get");
    assert_eq!(
        branch.len(),
        1,
        "expected 1 event under 'branch-x', got {}",
        branch.len()
    );
    let default = event_store
        .get("pm-domain", "", pm_root)
        .await
        .expect("get");
    assert_eq!(
        default.len(),
        0,
        "default-edition read must not see branch events; got {}",
        default.len()
    );
}

/// Persisting at a sequence the store already has returns
/// `CommandOutcome::Rejected { code: Internal, .. }`. This is the
/// classification `orchestrate_pm` reads in R2-15 step 5b -- the
/// caller routes to immediate-DLQ (NOT retry-then-DLQ) because the
/// underlying storage layer's sequence conflict is permanent at
/// this layer (the PM persist path doesn't retry on conflict; the
/// outer pm_retry loop is what handles that).
#[tokio::test]
async fn pm_persist_sequence_conflict_returns_rejected_internal() {
    let event_store = create_sqlite_event_store().await;
    let event_bus: Arc<dyn EventBus> = RecordingEventBus::new();
    let pm_root = Uuid::new_v4();

    // First persist at sequence 0 -- succeeds.
    let book = pm_event_book("pm-domain", pm_root, "corr-1", None, &[0]);
    let first = persist_pm_event_book(
        &(event_store.clone() as Arc<dyn EventStore>),
        &event_bus,
        "pm-domain",
        &book,
        "corr-1",
    )
    .await;
    assert!(matches!(first, CommandOutcome::Success(_)));

    // Second persist at sequence 0 -- conflict.
    let conflict = persist_pm_event_book(
        &(event_store.clone() as Arc<dyn EventStore>),
        &event_bus,
        "pm-domain",
        &book,
        "corr-1",
    )
    .await;
    match conflict {
        CommandOutcome::Rejected { code, message } => {
            assert_eq!(
                code,
                tonic::Code::Internal,
                "PM persist conflict must surface as Internal Rejected (immediate-DLQ classifier), got {code:?}: {message}"
            );
        }
        other => panic!("expected Rejected, got {other:?}"),
    }

    // No second event written.
    let persisted = event_store
        .get("pm-domain", "", pm_root)
        .await
        .expect("event_store.get");
    assert_eq!(
        persisted.len(),
        1,
        "sequence conflict must not double-persist, got {} events",
        persisted.len()
    );
}
