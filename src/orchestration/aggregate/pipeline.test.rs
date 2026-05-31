//! Unit tests for `execute_mode`'s extracted phase helpers.
//!
//! `execute_mode` itself is a private async orchestrator with no direct unit
//! test (only indirect coverage via the gherkin/integration suites). When it was
//! decomposed into named phase helpers, those helpers became individually
//! testable — that was a core motivation for the split. These tests pin each
//! helper's branching directly so mutations in them are caught.
//!
//! `use super::*` pulls in pipeline.rs's own items and its imports (traits,
//! proto types like `CommandBook`/`EventBook`/`MergeStrategy`, `Status`, `Uuid`,
//! the private helper fns, and `AggregateOperation`). The block below adds only
//! the extra names pipeline.rs does NOT already import, to avoid redundant-import
//! warnings under `-D warnings`.

use super::*;
use crate::proto::{
    command_page, event_page, AngzarrDeferredSequence, BusinessResponse, CommandPage, Cover,
    EventPage, PageHeader, Projection, Uuid as ProtoUuid,
};
use crate::storage::SourceInfo;
use prost_types::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ============================================================================
// Test fixtures
// ============================================================================

fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

fn cover(domain: &str, correlation_id: &str) -> Cover {
    Cover {
        domain: domain.to_string(),
        root: Some(proto_uuid(Uuid::new_v4())),
        correlation_id: correlation_id.to_string(),
        edition: None,
        ext: None,
    }
}

fn book_with_domain(domain: &str, correlation_id: &str) -> EventBook {
    EventBook {
        cover: Some(cover(domain, correlation_id)),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    }
}

/// An EventPage at `sequence`, committed unless `no_commit`/`cascade` say otherwise.
fn make_event_page(sequence: u32, no_commit: bool, cascade: Option<&str>) -> EventPage {
    EventPage {
        header: Some(PageHeader {
            sync_mode: None,
            sequence_type: Some(SequenceType::Sequence(sequence)),
        }),
        payload: Some(event_page::Payload::Event(Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        })),
        created_at: None,
        no_commit,
        cascade_id: cascade.map(String::from),
    }
}

/// A plain command carrying an explicit `Sequence` header (not deferred).
fn plain_command() -> CommandBook {
    CommandBook {
        cover: Some(cover("dest", "")),
        pages: vec![CommandPage {
            header: Some(PageHeader {
                sync_mode: None,
                sequence_type: Some(SequenceType::Sequence(0)),
            }),
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeStrict as i32,
        }],
    }
}

/// A saga-produced command carrying an `AngzarrDeferred` header whose `source`
/// cover is configurable (so we can exercise the empty-domain short-circuit).
fn deferred_command(source: Option<Cover>, source_seq: u32) -> CommandBook {
    CommandBook {
        cover: Some(cover("dest", "")),
        pages: vec![CommandPage {
            header: Some(PageHeader {
                sync_mode: None,
                sequence_type: Some(SequenceType::AngzarrDeferred(AngzarrDeferredSequence {
                    source,
                    source_seq,
                })),
            }),
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeStrict as i32,
        }],
    }
}

/// A `ClientLogic` whose `replay()` is the trait default (Unimplemented), used to
/// drive the commutative gate's degrade-to-STRICT path. `invoke`/`invoke_fact`
/// are never called by the helpers under test.
struct NoReplay;

#[async_trait]
impl ClientLogic for NoReplay {
    async fn invoke(&self, _cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        Err(Status::unimplemented("invoke not used in helper tests"))
    }

    async fn invoke_fact(&self, _ctx: FactContext) -> Result<EventBook, Status> {
        Err(Status::unimplemented(
            "invoke_fact not used in helper tests",
        ))
    }
    // replay() uses the trait default → Unimplemented.
}

/// Configurable `AggregateContext` exposing only what the helpers call.
#[derive(Default)]
struct TestCtx {
    cascade: Option<String>,
    /// Value returned by `check_deferred_idempotency`.
    deferred_cached: Option<EventBook>,
    /// Value returned by `post_persist`.
    post_persist_return: Vec<Projection>,
    post_persist_calls: Arc<AtomicUsize>,
    dlq_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl AggregateContext for TestCtx {
    async fn load_prior_events_with_divergence(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _temporal: &TemporalQuery,
        _explicit_divergence: Option<u32>,
    ) -> Result<EventBook, Status> {
        // Not exercised by the helpers under test.
        Ok(EventBook::default())
    }

    async fn persist_events(
        &self,
        _prior: &EventBook,
        _received: &EventBook,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _correlation_id: &str,
        _external_id: Option<&str>,
        _source_info: Option<&SourceInfo>,
    ) -> Result<PersistOutcome, Status> {
        // Not exercised by the helpers under test.
        Err(Status::unimplemented(
            "persist_events not used in helper tests",
        ))
    }

    async fn post_persist(&self, _events: &EventBook) -> Result<Vec<Projection>, Status> {
        self.post_persist_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.post_persist_return.clone())
    }

    fn cascade_id(&self) -> Option<&str> {
        self.cascade.as_deref()
    }

    async fn check_deferred_idempotency(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _deferred: &AngzarrDeferredSequence,
    ) -> Result<Option<EventBook>, Status> {
        Ok(self.deferred_cached.clone())
    }

    async fn send_to_dlq(
        &self,
        _command: &CommandBook,
        _expected_sequence: u32,
        _actual_sequence: u32,
        _domain: &str,
    ) {
        self.dlq_calls.fetch_add(1, Ordering::SeqCst);
    }
}

// ============================================================================
// extract_source_info
// ============================================================================

/// A non-deferred command (explicit Sequence) has no source provenance.
#[test]
fn test_extract_source_info_non_deferred_is_none() {
    assert!(extract_source_info(&plain_command()).is_none());
}

/// A deferred command whose source cover has an empty domain yields no source —
/// guards the `source.domain.is_empty()` short-circuit.
#[test]
fn test_extract_source_info_empty_source_domain_is_none() {
    let cmd = deferred_command(Some(cover("", "")), 5);
    assert!(extract_source_info(&cmd).is_none());
}

/// A deferred command with a valid source cover yields the source provenance,
/// with every field copied through (not defaulted).
#[test]
fn test_extract_source_info_valid_source() {
    let source_root = Uuid::new_v4();
    let mut src = cover("orders", "");
    src.root = Some(proto_uuid(source_root));
    let cmd = deferred_command(Some(src), 7);

    let info = extract_source_info(&cmd).expect("valid source should yield SourceInfo");
    assert_eq!(info.domain, "orders");
    assert_eq!(info.root, source_root);
    assert_eq!(info.seq, 7);
    assert_eq!(info.edition, ""); // no edition on the source cover
}

// ============================================================================
// should_pre_validate (pure truth table)
// ============================================================================

#[test]
fn test_should_pre_validate_strict_runs() {
    assert!(should_pre_validate(
        MergeStrategy::MergeStrict,
        false,
        false
    ));
}

#[test]
fn test_should_pre_validate_commutative_runs() {
    assert!(should_pre_validate(
        MergeStrategy::MergeCommutative,
        false,
        false
    ));
}

#[test]
fn test_should_pre_validate_manual_runs() {
    assert!(should_pre_validate(
        MergeStrategy::MergeManual,
        false,
        false
    ));
}

/// AGGREGATE_HANDLES owns its own concurrency — pre-validation is skipped.
#[test]
fn test_should_pre_validate_aggregate_handles_skipped() {
    assert!(!should_pre_validate(
        MergeStrategy::MergeAggregateHandles,
        false,
        false
    ));
}

/// Deferred (saga) commands skip pre-validation (sequence unknown until load).
#[test]
fn test_should_pre_validate_deferred_skipped() {
    assert!(!should_pre_validate(
        MergeStrategy::MergeStrict,
        true,
        false
    ));
}

/// Explicit divergence skips pre-validation (expected is the branch point).
#[test]
fn test_should_pre_validate_explicit_divergence_skipped() {
    assert!(!should_pre_validate(
        MergeStrategy::MergeStrict,
        false,
        true
    ));
}

// ============================================================================
// resolve_command_persist_outcome (pure)
// ============================================================================

#[test]
fn test_resolve_persist_outcome_persisted_is_not_noop() {
    let book = book_with_domain("orders", "c1");
    let (events, is_noop) =
        resolve_command_persist_outcome(PersistOutcome::Persisted(book)).expect("persisted ok");
    assert!(!is_noop, "Persisted must map to is_noop=false");
    assert_eq!(
        events.cover.expect("cover passed through").domain,
        "orders",
        "the persisted book must be returned, not a default"
    );
}

#[test]
fn test_resolve_persist_outcome_noop_is_noop() {
    let book = book_with_domain("orders", "c1");
    let (events, is_noop) =
        resolve_command_persist_outcome(PersistOutcome::NoOp(book)).expect("noop ok");
    assert!(is_noop, "NoOp must map to is_noop=true");
    assert_eq!(events.cover.expect("cover passed through").domain, "orders");
}

/// Commands never pass external_id, so a Duplicate outcome is an internal error.
#[test]
fn test_resolve_persist_outcome_duplicate_is_internal_error() {
    let err = resolve_command_persist_outcome(PersistOutcome::Duplicate {
        first_sequence: 0,
        last_sequence: 0,
    })
    .expect_err("Duplicate must be an error for commands");
    assert_eq!(err.code(), tonic::Code::Internal);
}

// ============================================================================
// apply_two_phase_transform
// ============================================================================

/// Non-cascade context: committed prior events pass through unchanged, the cover
/// is preserved, and there are no other-cascade uncommitted events.
#[tokio::test]
async fn test_apply_two_phase_non_cascade_passthrough() {
    let ctx = TestCtx::default(); // cascade_id() == None
    let mut prior = book_with_domain("orders", "c1");
    prior.pages = vec![make_event_page(0, false, None)];

    let (out, has_uncommitted) = apply_two_phase_transform(&ctx, &prior);

    assert!(
        !has_uncommitted,
        "no cascade context → no other-cascade work"
    );
    assert_eq!(
        out.cover
            .expect("cover preserved (not a default book)")
            .domain,
        "orders"
    );
    assert_eq!(out.pages.len(), 1, "committed page passes through");
}

/// Cascade context with no prior events: the cascade branch runs but finds no
/// uncommitted cascades, so the flag is false. Pins the `!is_empty()` polarity.
#[tokio::test]
async fn test_apply_two_phase_cascade_no_uncommitted_is_false() {
    let ctx = TestCtx {
        cascade: Some("cascade-A".to_string()),
        ..Default::default()
    };
    let prior = book_with_domain("orders", "c1"); // no pages

    let (_out, has_uncommitted) = apply_two_phase_transform(&ctx, &prior);

    assert!(
        !has_uncommitted,
        "empty prior → uncommitted_cascade_ids empty → flag false"
    );
}

// ============================================================================
// publish_unless_noop
// ============================================================================

/// NoOp: post_persist is skipped (H-16) and the result is empty.
#[tokio::test]
async fn test_publish_unless_noop_skips_on_noop() {
    let calls = Arc::new(AtomicUsize::new(0));
    let ctx = TestCtx {
        post_persist_calls: calls.clone(),
        post_persist_return: vec![Projection::default()],
        ..Default::default()
    };
    let book = book_with_domain("orders", "c1");

    let projections = publish_unless_noop(&ctx, &book, true).await.unwrap();

    assert!(projections.is_empty(), "NoOp must not publish");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "post_persist must be skipped"
    );
}

/// Non-NoOp: post_persist runs and its projections are returned.
#[tokio::test]
async fn test_publish_unless_noop_publishes_when_not_noop() {
    let calls = Arc::new(AtomicUsize::new(0));
    let ctx = TestCtx {
        post_persist_calls: calls.clone(),
        post_persist_return: vec![Projection::default()],
        ..Default::default()
    };
    let book = book_with_domain("orders", "c1");

    let projections = publish_unless_noop(&ctx, &book, false).await.unwrap();

    assert_eq!(
        projections.len(),
        1,
        "projections from post_persist returned"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "post_persist called once");
}

// ============================================================================
// try_deferred_idempotency_replay
// ============================================================================

/// Non-deferred command: short-circuits to None without touching the context.
#[tokio::test]
async fn test_try_deferred_replay_non_deferred_is_none() {
    let calls = Arc::new(AtomicUsize::new(0));
    let ctx = TestCtx {
        post_persist_calls: calls.clone(),
        ..Default::default()
    };
    let cmd = plain_command();

    let result =
        try_deferred_idempotency_replay(&ctx, &cmd, "dest", "angzarr", Uuid::new_v4(), "c")
            .await
            .unwrap();

    assert!(result.is_none());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "no republish for non-deferred"
    );
}

/// Deferred command with no cached result: returns None (not a duplicate).
#[tokio::test]
async fn test_try_deferred_replay_deferred_not_cached_is_none() {
    let ctx = TestCtx {
        deferred_cached: None,
        ..Default::default()
    };
    let cmd = deferred_command(Some(cover("orders", "")), 1);

    let result =
        try_deferred_idempotency_replay(&ctx, &cmd, "dest", "angzarr", Uuid::new_v4(), "c")
            .await
            .unwrap();

    assert!(result.is_none());
}

/// Deferred command already processed: returns the cached events, republishes
/// (post_persist), and stamps the in-flight correlation_id onto the empty cover.
#[tokio::test]
async fn test_try_deferred_replay_cached_returns_and_stamps_correlation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let ctx = TestCtx {
        // cached book has an empty correlation_id, as build_event_book produces.
        deferred_cached: Some(book_with_domain("orders", "")),
        post_persist_calls: calls.clone(),
        ..Default::default()
    };
    let cmd = deferred_command(Some(cover("orders", "")), 1);

    let response = try_deferred_idempotency_replay(
        &ctx,
        &cmd,
        "dest",
        "angzarr",
        Uuid::new_v4(),
        "corr-inflight",
    )
    .await
    .unwrap()
    .expect("cached deferred command must return a response");

    let events = response.events.expect("response carries the cached events");
    assert_eq!(
        events.cover.expect("cover present").correlation_id,
        "corr-inflight",
        "empty cached correlation_id must be stamped with the in-flight one (C-04)"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "cached result must be republished via post_persist"
    );
}

// ============================================================================
// enforce_merge_strategy
// ============================================================================

#[tokio::test]
async fn test_enforce_strict_non_deferred_rejects() {
    let ctx = TestCtx::default();
    let err = enforce_merge_strategy(
        &ctx,
        &plain_command(),
        MergeStrategy::MergeStrict,
        1,
        2,
        "dest",
        false,
    )
    .await
    .expect_err("STRICT mismatch must reject");
    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
}

/// STRICT is skipped for deferred commands (they never claim a sequence).
#[tokio::test]
async fn test_enforce_strict_deferred_is_ok() {
    let ctx = TestCtx::default();
    enforce_merge_strategy(
        &ctx,
        &plain_command(),
        MergeStrategy::MergeStrict,
        0,
        2,
        "dest",
        true,
    )
    .await
    .expect("STRICT is meaningless for deferred → Ok");
}

/// COMMUTATIVE proceeds (defers to the post-execution overlap check).
#[tokio::test]
async fn test_enforce_commutative_is_ok() {
    let ctx = TestCtx::default();
    enforce_merge_strategy(
        &ctx,
        &plain_command(),
        MergeStrategy::MergeCommutative,
        1,
        2,
        "dest",
        false,
    )
    .await
    .expect("COMMUTATIVE proceeds past the sequence gate");
}

/// MANUAL routes to the DLQ and aborts (non-retryable).
#[tokio::test]
async fn test_enforce_manual_sends_to_dlq_and_aborts() {
    let dlq = Arc::new(AtomicUsize::new(0));
    let ctx = TestCtx {
        dlq_calls: dlq.clone(),
        ..Default::default()
    };
    let err = enforce_merge_strategy(
        &ctx,
        &plain_command(),
        MergeStrategy::MergeManual,
        1,
        2,
        "dest",
        false,
    )
    .await
    .expect_err("MANUAL must abort");
    assert_eq!(err.code(), tonic::Code::Aborted);
    assert_eq!(dlq.load(Ordering::SeqCst), 1, "MANUAL must send to DLQ");
}

/// AGGREGATE_HANDLES does no coordinator-level validation.
#[tokio::test]
async fn test_enforce_aggregate_handles_is_ok() {
    let dlq = Arc::new(AtomicUsize::new(0));
    let ctx = TestCtx {
        dlq_calls: dlq.clone(),
        ..Default::default()
    };
    enforce_merge_strategy(
        &ctx,
        &plain_command(),
        MergeStrategy::MergeAggregateHandles,
        1,
        2,
        "dest",
        false,
    )
    .await
    .expect("AGGREGATE_HANDLES self-manages → Ok");
    assert_eq!(dlq.load(Ordering::SeqCst), 0, "must not touch the DLQ");
}

// ============================================================================
// enforce_cascade_conflict_gate / enforce_commutative_gate
// (thin wrappers over `merge`; pinned on deterministic paths — the conflict /
//  disjoint paths are covered by merge.test.rs)
// ============================================================================

/// With no uncommitted prior events there is no possible cascade conflict, so
/// the gate proceeds (`Ok`). Pins the NoConflict → Ok mapping.
#[tokio::test]
async fn test_cascade_gate_no_uncommitted_is_ok() {
    let business = NoReplay;
    let mut prior = book_with_domain("orders", "c1");
    prior.pages = vec![make_event_page(0, false, None)]; // committed only
    let received = book_with_domain("orders", "c1");

    enforce_cascade_conflict_gate(&business, &prior, &received)
        .await
        .expect("no uncommitted events → NoConflict → Ok");
}

/// When the aggregate can't replay (Unimplemented), the commutative check
/// degrades to STRICT: the gate rejects with FAILED_PRECONDITION and the plain
/// sequence-mismatch message (not the overlap variant). Pins the `Err` arm.
#[tokio::test]
async fn test_commutative_gate_replay_unimplemented_degrades_to_strict() {
    let business = NoReplay;
    let prior = book_with_domain("orders", "c1");
    let received = book_with_domain("orders", "c1");

    let err = enforce_commutative_gate(&business, &prior, &received, 1, 2)
        .await
        .expect_err("unimplemented replay must degrade to a rejection");
    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    assert!(
        err.message()
            .starts_with(crate::orchestration::errmsg::SEQUENCE_MISMATCH),
        "degraded path uses the plain mismatch message, got: {}",
        err.message()
    );
}

// ============================================================================
// AggregateOperation::name (trivial, but mutated — pin it)
// ============================================================================

#[test]
fn test_aggregate_operation_name() {
    let ctx = TestCtx::default();
    let business = NoReplay;
    let op = AggregateOperation {
        ctx: &ctx,
        business: &business,
        command_book: plain_command(),
    };
    assert_eq!(op.name(), "aggregate_command");
}
