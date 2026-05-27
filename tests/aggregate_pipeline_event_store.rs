//! Integration test: aggregate pipeline -> real SqliteEventStore
//!
//! Run with:
//! ```bash
//! cargo test --test aggregate_pipeline_event_store --features "test-utils" -- --nocapture
//! ```
//!
//! Closes the highest-leverage Category B drift gap from the R2-15
//! audit. The existing `storage_sqlite.rs` integration tests exercise
//! `EventStore.add` directly via a macro suite -- they prove the store
//! impl conforms to its trait contract. The unit tests for the
//! aggregate service (`services/aggregate.test.rs`) drive the pipeline
//! through `MockEventStore` -- they prove the orchestration calls the
//! trait correctly. Nothing bridged the two: a pipeline bug that
//! happens to be papered over by `MockEventStore` (which doesn't
//! enforce every SQL-level invariant the real store does) would not
//! surface in either layer.
//!
//! This test wires `AggregateService` to a real in-memory
//! `SqliteEventStore` and sends commands through `handle_command` --
//! the production gRPC entry point. After each command, it inspects
//! the persisted rows directly via `EventStore.get` to verify the
//! pipeline-specific shape:
//!
//! - Sequence stamping (first event at 0, second at 1).
//! - Sequence-conflict rejection (stale sequence -> FailedPrecondition).
//! - Edition propagation (command on a branch persists under the
//!   branch's edition, not the default).

#![cfg(feature = "test-utils")]

use std::collections::VecDeque;
use std::sync::Arc;

use prost_types::Any;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::Mutex;
use tonic::{Code, Request};
use uuid::Uuid;

use angzarr::bus::MockEventBus;
use angzarr::discovery::StaticServiceDiscovery;
use angzarr::orchestration::aggregate::{ClientLogic, FactContext};
use angzarr::proto::command_handler_coordinator_service_server::CommandHandlerCoordinatorService;
use angzarr::proto::{
    business_response, command_page, event_page, page_header, BusinessResponse, CascadeErrorMode,
    CommandBook, CommandPage, CommandRequest, ContextualCommand, Cover, Edition, EventBook,
    EventPage, MergeStrategy, PageHeader, SyncMode, Uuid as ProtoUuid,
};
use angzarr::repository::SnapshotRepository;
use angzarr::services::AggregateService;
use angzarr::storage::{EventStore, SqliteEventStore, SqliteSnapshotStore};
use async_trait::async_trait;

// ============================================================================
// Test fixtures
// ============================================================================

/// Minimal `ClientLogic` test double: returns pre-enqueued event books
/// in FIFO order. Each command invocation pops the next queued
/// response. Fact invocations get their own queue.
struct QueuedClientLogic {
    responses: Mutex<VecDeque<EventBook>>,
    fact_responses: Mutex<VecDeque<EventBook>>,
    invocations: Mutex<Vec<ContextualCommand>>,
}

impl QueuedClientLogic {
    fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            fact_responses: Mutex::new(VecDeque::new()),
            invocations: Mutex::new(Vec::new()),
        }
    }

    async fn enqueue(&self, events: EventBook) {
        self.responses.lock().await.push_back(events);
    }
}

#[async_trait]
impl ClientLogic for QueuedClientLogic {
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, tonic::Status> {
        self.invocations.lock().await.push(cmd);
        let events = self.responses.lock().await.pop_front().unwrap_or_default();
        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
    }

    async fn invoke_fact(&self, ctx: FactContext) -> Result<EventBook, tonic::Status> {
        Ok(self
            .fact_responses
            .lock()
            .await
            .pop_front()
            .unwrap_or(ctx.facts))
    }
}

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

async fn create_sqlite_snapshot_repo() -> Arc<SnapshotRepository> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("connect SQLite snapshot pool");
    sqlx::migrate!("./migrations/sqlite")
        .run(&pool)
        .await
        .expect("run sqlite migrations");
    Arc::new(SnapshotRepository::new(Arc::new(SqliteSnapshotStore::new(
        pool,
    ))))
}

async fn create_service_with_sqlite() -> (
    AggregateService,
    Arc<QueuedClientLogic>,
    Arc<SqliteEventStore>,
) {
    let event_store = create_sqlite_event_store().await;
    let snapshot_repo = create_sqlite_snapshot_repo().await;
    let business = Arc::new(QueuedClientLogic::new());
    let event_bus = Arc::new(MockEventBus::new());
    let discovery = Arc::new(StaticServiceDiscovery::new());

    let service = AggregateService::with_business_logic(
        event_store.clone(),
        snapshot_repo,
        business.clone(),
        event_bus,
        discovery,
    );
    (service, business, event_store)
}

fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

fn cover(domain: &str, root: Uuid, edition: Option<&str>) -> Cover {
    Cover {
        domain: domain.to_string(),
        root: Some(proto_uuid(root)),
        correlation_id: String::new(),
        edition: edition.map(|name| Edition {
            name: name.to_string(),
            divergences: vec![],
        }),
        ext: None,
    }
}

fn command_book(domain: &str, root: Uuid, sequence: u32, edition: Option<&str>) -> CommandBook {
    CommandBook {
        cover: Some(cover(domain, root, edition)),
        pages: vec![CommandPage {
            header: Some(PageHeader {
                sync_mode: None,
                sequence_type: Some(page_header::SequenceType::Sequence(sequence)),
            }),
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
    }
}

fn event_page(seq: u32) -> EventPage {
    EventPage {
        header: Some(PageHeader {
            sync_mode: None,
            sequence_type: Some(page_header::SequenceType::Sequence(seq)),
        }),
        payload: Some(event_page::Payload::Event(Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        })),
        created_at: None,
        ..Default::default()
    }
}

fn event_book(domain: &str, root: Uuid, edition: Option<&str>, pages: Vec<EventPage>) -> EventBook {
    EventBook {
        cover: Some(cover(domain, root, edition)),
        pages,
        snapshot: None,
        ..Default::default()
    }
}

fn send(command_book: CommandBook) -> Request<CommandRequest> {
    Request::new(CommandRequest {
        command: Some(command_book),
        sync_mode: SyncMode::Async as i32,
        cascade_error_mode: CascadeErrorMode::CascadeErrorFailFast.into(),
        cascade_id: None,
    })
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

/// A single command flows through the pipeline and persists exactly
/// one event at sequence 0 in the SQLite event store.
///
/// What this catches that lib tests don't: the pipeline's persist
/// call goes through `EventStore.add`'s real SQL path -- column
/// types, transaction handling, sequence column constraints. A bug
/// in the pipeline that produces a malformed insert payload would
/// surface here but not in MockEventStore-based unit tests.
#[tokio::test]
async fn pipeline_persists_single_event_at_sequence_zero() {
    let (service, business, store) = create_service_with_sqlite().await;
    let root = Uuid::new_v4();

    business
        .enqueue(event_book("orders", root, None, vec![event_page(0)]))
        .await;

    let response = service
        .handle_command(send(command_book("orders", root, 0, None)))
        .await;
    assert!(response.is_ok(), "pipeline returned: {:?}", response.err());

    let persisted = store
        .get("orders", "", root)
        .await
        .expect("event_store.get");
    assert_eq!(persisted.len(), 1, "expected exactly 1 persisted event");
    assert_eq!(event_sequence_num(&persisted[0]), 0);
}

/// Two commands against the same root produce two events with
/// monotonically increasing sequences (0, 1). The pipeline must
/// re-read the prior state between commands and stamp the next
/// sequence correctly -- a regression in `next_sequence` calculation
/// would surface here.
#[tokio::test]
async fn pipeline_increments_sequence_across_two_commands() {
    let (service, business, store) = create_service_with_sqlite().await;
    let root = Uuid::new_v4();

    business
        .enqueue(event_book("orders", root, None, vec![event_page(0)]))
        .await;
    business
        .enqueue(event_book("orders", root, None, vec![event_page(1)]))
        .await;

    let r1 = service
        .handle_command(send(command_book("orders", root, 0, None)))
        .await;
    assert!(r1.is_ok(), "first command failed: {:?}", r1.err());
    let r2 = service
        .handle_command(send(command_book("orders", root, 1, None)))
        .await;
    assert!(r2.is_ok(), "second command failed: {:?}", r2.err());

    let persisted = store
        .get("orders", "", root)
        .await
        .expect("event_store.get");
    assert_eq!(persisted.len(), 2, "expected 2 persisted events");
    assert_eq!(event_sequence_num(&persisted[0]), 0);
    assert_eq!(event_sequence_num(&persisted[1]), 1);
}

/// A stale command (sequence 0 when the aggregate is already at 1)
/// must be rejected with `FailedPrecondition` -- the framework's
/// sequence-conflict signal -- and must NOT add a second event at
/// sequence 0. Mirrors the production single-sequence-check contract
/// against a real store rather than a mock.
#[tokio::test]
async fn pipeline_rejects_stale_sequence_against_sqlite_store() {
    let (service, business, store) = create_service_with_sqlite().await;
    let root = Uuid::new_v4();

    // Persist event at sequence 0 via a successful first command.
    business
        .enqueue(event_book("orders", root, None, vec![event_page(0)]))
        .await;
    let first = service
        .handle_command(send(command_book("orders", root, 0, None)))
        .await;
    assert!(first.is_ok());

    // Stale command at sequence 0 -- the store already has an event
    // at 0, so this must fail.
    business
        .enqueue(event_book("orders", root, None, vec![event_page(0)]))
        .await;
    let stale = service
        .handle_command(send(command_book("orders", root, 0, None)))
        .await;
    assert!(stale.is_err(), "stale-sequence command must be rejected");
    let status = stale.unwrap_err();
    assert_eq!(
        status.code(),
        Code::FailedPrecondition,
        "stale-sequence rejection must be FailedPrecondition, got {:?}: {}",
        status.code(),
        status.message()
    );

    // No additional events should have been written.
    let persisted = store
        .get("orders", "", root)
        .await
        .expect("event_store.get");
    assert_eq!(
        persisted.len(),
        1,
        "stale-sequence rejection must not add a second event; got {} events",
        persisted.len()
    );
}

/// A command with `cover.edition = "branch-a"` persists its event
/// under edition "branch-a", not the default edition. This pins the
/// pipeline's edition-extraction-and-propagation logic against the
/// real store's edition column.
#[tokio::test]
async fn pipeline_propagates_edition_to_event_store_writes() {
    let (service, business, store) = create_service_with_sqlite().await;
    let root = Uuid::new_v4();

    business
        .enqueue(event_book(
            "orders",
            root,
            Some("branch-a"),
            vec![event_page(0)],
        ))
        .await;

    let response = service
        .handle_command(send(command_book("orders", root, 0, Some("branch-a"))))
        .await;
    assert!(response.is_ok(), "pipeline returned: {:?}", response.err());

    // Event should be visible under the branch edition, NOT the default.
    let branch = store.get("orders", "branch-a", root).await.expect("get");
    assert_eq!(
        branch.len(),
        1,
        "expected 1 event under edition 'branch-a', got {}",
        branch.len()
    );
    let default = store.get("orders", "", root).await.expect("get default");
    assert_eq!(
        default.len(),
        0,
        "default-edition read must NOT see branch events; got {}",
        default.len()
    );
}
