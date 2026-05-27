//! Tests for the gRPC aggregate context.
//!
//! The `should_skip_post_persist` predicate that used to live here moved to
//! `super::super::sync_policy` (C-06), so its tests are now in
//! `sync_policy.test.rs` — they are the single source of truth that drives
//! both the local and gRPC `post_persist` short-circuit.
//!
//! Tests below pin gRPC-context-specific load semantics (R2-SNAP-4 etc.).

use super::*;
use crate::bus::MockEventBus;
use crate::discovery::StaticServiceDiscovery;
use crate::proto::Snapshot;
use crate::repository::SnapshotRepository;
use crate::storage::mock::{MockEventStore, MockSnapshotStore};
use crate::storage::SnapshotStore;
use crate::test_utils::make_event_page;

fn build_ctx_with_stores(
    event_store: Arc<MockEventStore>,
    snapshot_store: Arc<MockSnapshotStore>,
) -> GrpcAggregateContext {
    let snapshot_repo = Arc::new(SnapshotRepository::new(snapshot_store));
    GrpcAggregateContext::new(
        event_store,
        snapshot_repo,
        Arc::new(StaticServiceDiscovery::new()),
        Arc::new(MockEventBus::new()),
    )
}

// ============================================================================
// R2-SNAP-4: load_prior_events_with_divergence honors snapshot when present
// ============================================================================

/// Standard (no-divergence) load uses the snapshot when one exists for
/// the aggregate. Regression guard for the "snapshot exists, load it;
/// layer events from snapshot.sequence + 1" contract.
#[tokio::test]
async fn test_load_standard_path_uses_snapshot_when_present() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let root = Uuid::new_v4();
    let edition = "";

    event_store
        .add(
            "orders",
            edition,
            root,
            (0..5).map(make_event_page).collect(),
            "",
            None,
            None,
        )
        .await
        .unwrap();
    snapshot_store
        .put(
            "orders",
            edition,
            root,
            Snapshot {
                sequence: 2,
                state: None,
                retention: crate::proto::SnapshotRetention::RetentionDefault as i32,
                created_at: None,
            },
        )
        .await
        .unwrap();

    let ctx = build_ctx_with_stores(event_store, snapshot_store);
    let book = ctx
        .load_prior_events_with_divergence(
            "orders",
            edition,
            root,
            &super::TemporalQuery::Current,
            None,
        )
        .await
        .unwrap();

    assert_eq!(
        book.pages.len(),
        2,
        "snapshot at seq=2 → only events 3,4 should be loaded"
    );
    assert!(
        book.snapshot.is_some(),
        "loaded EventBook must carry the snapshot"
    );
    assert_eq!(book.snapshot.unwrap().sequence, 2);
}

/// R2-SNAP-4 contract: explicit_divergence load uses the snapshot
/// when one exists for the branch's edition. Pre-fix: the
/// explicit_divergence branch unconditionally skipped the snapshot
/// and replayed from the divergence point.
#[tokio::test]
async fn test_load_explicit_divergence_uses_snapshot_when_present() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let root = Uuid::new_v4();
    let edition = "branch-v2";

    // Branch has events 0..5 and a snapshot at sequence 2.
    event_store
        .add(
            "orders",
            edition,
            root,
            (0..5).map(make_event_page).collect(),
            "",
            None,
            None,
        )
        .await
        .unwrap();
    snapshot_store
        .put(
            "orders",
            edition,
            root,
            Snapshot {
                sequence: 2,
                state: None,
                retention: crate::proto::SnapshotRetention::RetentionDefault as i32,
                created_at: None,
            },
        )
        .await
        .unwrap();

    let ctx = build_ctx_with_stores(event_store, snapshot_store);
    let book = ctx
        .load_prior_events_with_divergence(
            "orders",
            edition,
            root,
            &super::TemporalQuery::Current,
            Some(0), // explicit divergence at 0 (branch starts at sequence 0)
        )
        .await
        .unwrap();

    assert!(
        book.snapshot.is_some(),
        "explicit_divergence must NOT skip the snapshot when one exists"
    );
    assert_eq!(book.snapshot.as_ref().unwrap().sequence, 2);
    assert_eq!(
        book.pages.len(),
        2,
        "snapshot at seq=2 → only events 3,4 loaded; pre-fix would have returned all 5"
    );
}

/// R2-SNAP-4 regression guard: when no snapshot exists for the
/// branch, explicit_divergence falls back to the get_with_divergence
/// path (the legacy behavior). Required so the fix doesn't change
/// behavior for the common "new branch, no snapshot yet" case the
/// path was originally designed for.
#[tokio::test]
async fn test_load_explicit_divergence_falls_back_when_no_snapshot() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let root = Uuid::new_v4();
    let edition = "branch-no-snap";

    event_store
        .add(
            "orders",
            edition,
            root,
            (0..3).map(make_event_page).collect(),
            "",
            None,
            None,
        )
        .await
        .unwrap();

    let ctx = build_ctx_with_stores(event_store, snapshot_store);
    let book = ctx
        .load_prior_events_with_divergence(
            "orders",
            edition,
            root,
            &super::TemporalQuery::Current,
            Some(0),
        )
        .await
        .unwrap();

    assert!(
        book.snapshot.is_none(),
        "no snapshot exists; loaded book must reflect that"
    );
    // The exact page count depends on the mock's get_with_divergence
    // implementation; assert non-empty to confirm the path ran.
    assert!(
        !book.pages.is_empty(),
        "fallback path must still produce events"
    );
}

// ============================================================================
// publish_aggregate_sequence_mismatch_dlq (R2-15 step 4 seam, refactored
// 2026-05-27 to be testable without a full GrpcAggregateContext)
// ============================================================================

use crate::dlq::{
    AngzarrDeadLetter as DlAngzarrDeadLetter, DeadLetterPayload, DeadLetterPublisher, DlqError,
    RejectionDetails,
};
use crate::proto::Cover;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::Mutex;

/// Captures dead letters so the test can inspect the constructed
/// shape that flows from the MergeManual sequence-mismatch path.
#[derive(Default)]
struct CapturingDlqPublisher {
    captured: Mutex<Vec<DlAngzarrDeadLetter>>,
    publish_calls: AtomicU32,
}

#[async_trait]
impl DeadLetterPublisher for CapturingDlqPublisher {
    async fn publish(&self, dead_letter: DlAngzarrDeadLetter) -> Result<(), DlqError> {
        self.publish_calls.fetch_add(1, Ordering::SeqCst);
        self.captured.lock().await.push(dead_letter);
        Ok(())
    }
}

fn cmd_for(domain: &str, correlation_id: &str) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: None,
            correlation_id: correlation_id.to_string(),
            edition: None,
            ext: None,
        }),
        pages: vec![],
    }
}

/// `publish_aggregate_sequence_mismatch_dlq` constructs an
/// `AngzarrDeadLetter` with the expected shape and forwards it to
/// the publisher. The MergeManual contract: sequence mismatch
/// details + Command payload + source_component_type "aggregate".
#[tokio::test]
async fn publish_aggregate_sequence_mismatch_dlq_builds_correct_shape() {
    let capture = Arc::new(CapturingDlqPublisher::default());
    let publisher: Arc<dyn DeadLetterPublisher> = capture.clone();

    let command = cmd_for("orders", "corr-1");
    publish_aggregate_sequence_mismatch_dlq(
        &publisher,
        &command,
        3, // expected
        7, // actual
        "orders",
        "aggregate-orders",
    )
    .await;

    assert_eq!(
        capture.publish_calls.load(Ordering::SeqCst),
        1,
        "publisher.publish must be called exactly once"
    );
    let entries = capture.captured.lock().await.clone();
    assert_eq!(entries.len(), 1);

    let dl = &entries[0];
    assert_eq!(dl.source_component, "aggregate-orders");
    assert_eq!(dl.source_component_type, "aggregate");
    match &dl.payload {
        DeadLetterPayload::Command(_) => {}
        other => panic!("expected Command payload, got {other:?}"),
    }
    match &dl.rejection_details {
        Some(RejectionDetails::SequenceMismatch(details)) => {
            assert_eq!(details.expected_sequence, 3);
            assert_eq!(details.actual_sequence, 7);
        }
        other => panic!("expected SequenceMismatch details, got {other:?}"),
    }
}

/// `send_to_dlq` on the full `GrpcAggregateContext` delegates to the
/// free fn. Same shape as the test above, but routed through the
/// context method to pin the wrapper.
#[tokio::test]
async fn send_to_dlq_on_context_delegates_to_free_fn() {
    let publisher = Arc::new(CapturingDlqPublisher::default());
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store: Arc<MockSnapshotStore> = Arc::new(MockSnapshotStore::default());
    let snapshot_repo = Arc::new(SnapshotRepository::new(snapshot_store));
    let ctx = GrpcAggregateContext::new(
        event_store,
        snapshot_repo,
        Arc::new(StaticServiceDiscovery::new()),
        Arc::new(MockEventBus::new()),
    )
    .with_dlq_publisher(publisher.clone())
    .with_component_name("aggregate-orders");

    use crate::orchestration::aggregate::traits::AggregateContext;
    ctx.send_to_dlq(&cmd_for("orders", "corr-2"), 1, 4, "orders")
        .await;

    let calls = publisher.publish_calls.load(Ordering::SeqCst);
    assert_eq!(calls, 1, "context method must forward to publisher");
    let entries = publisher.captured.lock().await.clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source_component, "aggregate-orders");
    assert_eq!(entries[0].source_component_type, "aggregate");
}
