//! Aggregate command execution pipeline abstraction.
//!
//! This module implements the core command processing flow for event-sourced
//! aggregates. Commands flow through: parse → load → validate → invoke →
//! persist → publish.
//!
//! # Sequence Validation and Merge Strategies
//!
//! Commands include an `expected_sequence` indicating what aggregate state they
//! were prepared against. When this doesn't match the current `actual_sequence`,
//! a concurrent write occurred. How we handle this depends on the merge strategy:
//!
//! | Strategy | Behavior | Use Case |
//! |----------|----------|----------|
//! | **STRICT** | Return FAILED_PRECONDITION, retry with fresh state | Default, most operations |
//! | **COMMUTATIVE** | Check field overlap; proceed if disjoint, else retry | High-concurrency aggregates |
//! | **MANUAL** | Send to DLQ for human review | Conflict-sensitive operations |
//! | **AGGREGATE_HANDLES** | Skip validation, let aggregate decide | Custom concurrency control |
//!
//! STRICT is the safest default: always retry on mismatch. COMMUTATIVE optimizes
//! for throughput when concurrent writes often touch different fields (e.g.,
//! counters, independent properties). MANUAL is for operations where automatic
//! retry could cause business problems (e.g., financial transactions).
//!
//! # Architecture
//!
//! - `AggregateContext`: Storage access and post-persist hooks (local vs gRPC impl)
//! - `ClientLogic`: Business logic invocation (gRPC client to aggregate handler)
//! - `execute_command_pipeline`: The main execution flow
//! - `try_commutative_merge`: Field-level conflict detection for COMMUTATIVE mode
//!
//! # Module Structure
//!
//! - `local/`: SQLite-backed storage with static service discovery
//! - `grpc/`: Remote storage with K8s service discovery

// tonic::Status is large by design - it carries error details for gRPC
#![allow(clippy::result_large_err)]

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use std::sync::Arc;

use async_trait::async_trait;
use backon::ExponentialBuilder;
use tokio::sync::Mutex;
use tonic::Status;
use uuid::Uuid;

use crate::proto::{
    command_handler_service_client::CommandHandlerServiceClient, page_header::SequenceType,
    AngzarrDeferredSequence, BusinessResponse, CommandBook, CommandResponse, ContextualCommand,
    EventBook, MergeStrategy, Projection, ReplayRequest,
};
use crate::proto_ext::{
    calculate_set_next_seq, CommandBookExt, CoverExt, EventBookExt, EventPageExt,
};
use crate::utils::response_builder::extract_events_from_response;
use crate::utils::retry::{is_retryable_status, run_with_retry, RetryOutcome, RetryableOperation};

/// How to load prior events.
#[derive(Debug, Clone)]
pub enum TemporalQuery {
    /// Current state (latest events, snapshot-optimized).
    Current,
    /// Events up to a specific sequence number (inclusive).
    AsOfSequence(u32),
    /// Events up to a specific timestamp.
    AsOfTimestamp(String),
}

/// Pipeline execution mode.
#[derive(Debug, Clone)]
pub enum PipelineMode {
    /// Normal execution: validate → invoke → persist → post-persist.
    Execute,
    /// Speculative: load temporal state → invoke → return (no persist/publish).
    Speculative {
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<String>,
    },
}

/// Context for aggregate command pipeline.
///
/// Implementations provide storage access and post-persist behavior.
/// client logic invocation is always via gRPC and handled by the pipeline.
///
/// All domain-scoped methods take `domain` and `edition` as separate parameters.
/// Domain is the bare aggregate domain (`"order"`, `"cart"`).
/// Edition identifies the timeline (`"angzarr"` for main, named editions for forks).
#[async_trait]
pub trait AggregateContext: Send + Sync {
    /// Load prior events for the aggregate.
    async fn load_prior_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status>;

    /// Persist new events to storage.
    ///
    /// Compares `prior` (sent to client logic) with `received` (returned by client logic)
    /// to determine what to persist:
    /// - If snapshots differ (by state comparison), persist the new snapshot
    /// - If pages differ (new pages in received), persist the new pages
    /// - If identical, no-op
    async fn persist_events(
        &self,
        prior: &EventBook,
        received: &EventBook,
        domain: &str,
        edition: &str,
        root: Uuid,
        correlation_id: &str,
    ) -> Result<EventBook, Status>;

    /// Publish to event bus AND call sync projectors via service discovery.
    /// Returns projections from sync projectors.
    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status>;

    /// Optional: pre-validate sequence before loading events (gRPC fast-path).
    /// On mismatch, may return Status with EventBook in details.
    async fn pre_validate_sequence(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _expected: u32,
    ) -> Result<(), Status> {
        Ok(())
    }

    /// Optional: transform events after loading (e.g., upcasting).
    async fn transform_events(
        &self,
        _domain: &str,
        events: EventBook,
    ) -> Result<EventBook, Status> {
        Ok(events)
    }

    /// Optional: send a command to the dead letter queue.
    /// Called when MERGE_MANUAL encounters a sequence mismatch.
    /// Default implementation logs a warning.
    async fn send_to_dlq(
        &self,
        _command: &CommandBook,
        expected_sequence: u32,
        actual_sequence: u32,
        domain: &str,
    ) {
        tracing::warn!(
            domain = %domain,
            expected = expected_sequence,
            actual = actual_sequence,
            "DLQ not configured, dropping command"
        );
    }

    /// Check if an external_id has already been used for fact injection.
    ///
    /// Returns `Some((first_seq, last_seq))` if already claimed, None if available.
    /// Default implementation returns None (no idempotency checking).
    async fn check_fact_idempotency(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _external_id: &str,
    ) -> Result<Option<(u32, u32)>, Status> {
        Ok(None)
    }

    /// Record a fact injection for idempotency tracking.
    ///
    /// Called after facts are persisted to record the external_id claim.
    /// Default implementation does nothing.
    async fn record_fact_idempotency(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _external_id: &str,
        _first_sequence: u32,
        _last_sequence: u32,
    ) -> Result<(), Status> {
        Ok(())
    }

    /// Check if a saga-produced command has already been processed.
    ///
    /// For commands with `angzarr_deferred` sequences, checks if events exist
    /// with matching source info (edition, domain, root, sequence).
    /// Returns `Some(events)` if already processed, None if new.
    ///
    /// Default implementation returns None (no idempotency checking).
    async fn check_deferred_idempotency(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _deferred: &AngzarrDeferredSequence,
    ) -> Result<Option<EventBook>, Status> {
        Ok(None)
    }
}

/// Context for fact event handling.
///
/// Contains the fact events to record and the aggregate's prior events.
#[derive(Debug, Clone)]
pub struct FactContext {
    /// The fact events to record (with ExternalDeferredSequence markers in PageHeader).
    pub facts: EventBook,
    /// Prior events for this aggregate root (for state reconstruction).
    pub prior_events: Option<EventBook>,
}

/// Abstraction for aggregate client logic invocation.
///
/// Decouples the command pipeline from the transport used to call client logic.
/// Implementations may use gRPC (over TCP, UDS), in-process trait calls, etc.
#[async_trait]
pub trait ClientLogic: Send + Sync {
    /// Invoke client logic with prior events and a command.
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status>;

    /// Invoke client logic to handle fact events.
    ///
    /// Called when fact events (with ExternalDeferredSequence markers) are injected.
    /// The aggregate updates its state based on the facts and returns
    /// events to persist. The coordinator will assign real sequence numbers.
    ///
    /// Default: Returns the facts unchanged (pass-through).
    async fn invoke_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
        // Default: pass through facts unchanged
        Ok(ctx.facts)
    }

    /// Replay events to compute state for COMMUTATIVE merge detection.
    ///
    /// Returns the aggregate's state at the given event sequence as a protobuf `Any`.
    /// The state can then be compared field-by-field to detect conflicts.
    ///
    /// Default: Returns Unimplemented, causing COMMUTATIVE to degrade to STRICT behavior.
    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        let _ = events;
        Err(Status::unimplemented(
            "Replay not implemented. Aggregate must implement replay() for MERGE_COMMUTATIVE field detection.",
        ))
    }
}

/// Factory for creating per-domain aggregate contexts.
///
/// Captures long-lived dependencies (storage, event bus, discovery) and produces
/// a fresh `AggregateContext` for each command execution. Local and gRPC modes
/// provide different implementations.
///
/// One factory per aggregate domain, matching the saga/PM pattern:
/// - `SagaContextFactory` → one per saga
/// - `PMContextFactory` → one per process manager
/// - `AggregateContextFactory` → one per aggregate domain
pub trait AggregateContextFactory: Send + Sync {
    /// Create an aggregate context for command execution.
    fn create(&self) -> Arc<dyn AggregateContext>;

    /// The domain this factory handles.
    fn domain(&self) -> &str;

    /// The client logic for this domain's business rules.
    fn client_logic(&self) -> Arc<dyn ClientLogic>;
}

/// client logic invocation via gRPC `AggregateClient`.
///
/// Wraps a tonic `AggregateClient` channel (TCP, UDS, or duplex).
pub struct GrpcBusinessLogic {
    client: Mutex<CommandHandlerServiceClient<tonic::transport::Channel>>,
}

impl GrpcBusinessLogic {
    /// Wrap a gRPC aggregate client as a `ClientLogic` implementation.
    pub fn new(client: CommandHandlerServiceClient<tonic::transport::Channel>) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }
}

#[async_trait]
impl ClientLogic for GrpcBusinessLogic {
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        Ok(self.client.lock().await.handle(cmd).await?.into_inner())
    }

    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        let request = ReplayRequest {
            events: events.pages.clone(),
            base_snapshot: events.snapshot.clone(),
        };
        let response = self.client.lock().await.replay(request).await?.into_inner();
        response
            .state
            .ok_or_else(|| Status::internal(crate::orchestration::errmsg::REPLAY_MISSING_STATE))
    }
}

/// Parse domain and root UUID from a CommandBook cover.
///
/// Validates domain format before returning.
pub fn parse_command_cover(command: &CommandBook) -> Result<(String, Uuid), Status> {
    let cover = command.cover.as_ref().ok_or_else(|| {
        Status::invalid_argument(crate::orchestration::errmsg::COMMAND_BOOK_MISSING_COVER)
    })?;

    let domain = cover.domain.clone();
    crate::validation::validate_domain(&domain)?;

    let root = cover.root.as_ref().ok_or_else(|| {
        Status::invalid_argument(crate::orchestration::errmsg::COVER_MISSING_ROOT)
    })?;

    let root_uuid = Uuid::from_slice(&root.value).map_err(|e| {
        Status::invalid_argument(format!("{}{e}", crate::orchestration::errmsg::INVALID_UUID))
    })?;

    Ok((domain, root_uuid))
}

/// Extract expected sequence from the first command page.
///
/// Handles both explicit sequences and deferred sequences:
/// - Explicit sequence: returns the sequence number
/// - Deferred sequences: returns 0 (framework will stamp on receipt)
pub fn extract_command_sequence(command: &CommandBook) -> u32 {
    use crate::proto::page_header::SequenceType;
    command
        .pages
        .first()
        .and_then(|p| p.header.as_ref())
        .and_then(|h| h.sequence_type.as_ref())
        .map(|st| match st {
            SequenceType::Sequence(seq) => *seq,
            // Deferred sequences don't have a fixed sequence yet
            SequenceType::ExternalDeferred(_) | SequenceType::AngzarrDeferred(_) => 0,
        })
        .unwrap_or(0)
}

/// Check if command has a deferred sequence (saga-produced or external).
///
/// Commands with deferred sequences need special handling:
/// - Framework stamps actual sequence before execution
/// - Idempotency checking may be required
fn has_deferred_sequence(command: &CommandBook) -> bool {
    command
        .pages
        .first()
        .and_then(|p| p.header.as_ref())
        .and_then(|h| h.sequence_type.as_ref())
        .map(|st| {
            matches!(
                st,
                SequenceType::AngzarrDeferred(_) | SequenceType::ExternalDeferred(_)
            )
        })
        .unwrap_or(false)
}

/// Extract AngzarrDeferredSequence from command if present.
///
/// Used for idempotency checking - the source info uniquely identifies
/// the saga invocation that produced this command.
fn extract_angzarr_deferred(command: &CommandBook) -> Option<&AngzarrDeferredSequence> {
    command
        .pages
        .first()
        .and_then(|p| p.header.as_ref())
        .and_then(|h| h.sequence_type.as_ref())
        .and_then(|st| match st {
            SequenceType::AngzarrDeferred(ad) => Some(ad),
            _ => None,
        })
}

/// Stamp actual sequence onto all command pages with deferred sequences.
///
/// Converts deferred sequences to explicit sequences while preserving
/// the provenance information in the header.
fn stamp_deferred_sequences(command: &mut CommandBook, actual_sequence: u32) {
    for (i, page) in command.pages.iter_mut().enumerate() {
        if let Some(header) = &mut page.header {
            if let Some(st) = &header.sequence_type {
                if matches!(
                    st,
                    SequenceType::AngzarrDeferred(_) | SequenceType::ExternalDeferred(_)
                ) {
                    // Stamp the actual sequence while preserving deferred info
                    // The sequence becomes actual_sequence + page_index
                    header.sequence_type = Some(SequenceType::Sequence(actual_sequence + i as u32));
                }
            }
        }
    }
}

/// Default edition name for the canonical (main) timeline.
///
/// Re-exported from proto_ext::constants for backwards compatibility.
pub use crate::proto_ext::constants::DEFAULT_EDITION;

/// Extract and validate edition name from a CommandBook's Cover.
///
/// Returns the edition name from `Cover.edition`, defaulting to [`DEFAULT_EDITION`]
/// when absent or empty. Validates edition format.
fn extract_edition(command_book: &CommandBook) -> Result<String, Status> {
    let edition = command_book.edition().to_string();
    crate::validation::validate_edition(&edition)?;
    Ok(edition)
}

/// Result of commutative merge check.
#[derive(Debug)]
enum CommutativeMergeResult {
    /// Fields changed by intervening events don't overlap with command's changes.
    Disjoint,
    /// Fields overlap - command must retry with fresh state.
    Overlap,
}

/// Check for field overlap after command execution (post-execution commutative merge).
///
/// # Why Post-Execution Check
///
/// Strict sequence validation rejects commands whenever `expected != actual`, even
/// when the intervening events touched completely different fields. This is safe
/// but wasteful — many concurrent writes are actually non-conflicting.
///
/// Commutative merge detects when changes are **disjoint**: if events from
/// `expected` to `actual` only touched `field_a`, and our command only changed
/// `field_b`, there's no conflict. We can persist without retry.
///
/// # Algorithm
///
/// 1. Replay aggregate state at `expected` sequence (what command assumed)
/// 2. Replay aggregate state at `actual` sequence (current reality)
/// 3. Replay aggregate state after applying command's events
/// 4. Diff (expected, actual) → fields changed by intervening events
/// 5. Diff (actual, after_command) → fields changed by this command
/// 6. If disjoint → persist; if overlap → reject and retry
///
/// # Why Check Post-Execution
///
/// We check AFTER command execution because we can observe what fields the command
/// actually changed, rather than trying to predict from command metadata. This is
/// more accurate and requires no annotations or naming conventions.
///
/// # Graceful Degradation
///
/// If Replay RPC fails (unimplemented, timeout, etc.), we degrade to STRICT
/// behavior. This is conservative: we'd rather retry unnecessarily than risk
/// incorrect merges.
///
/// Returns:
/// - `Ok(Disjoint)` if changes don't overlap → safe to persist
/// - `Ok(Overlap)` if changes overlap → must retry
/// - `Err(_)` if Replay unavailable → degrade to STRICT behavior
async fn check_commutative_overlap(
    business: &dyn ClientLogic,
    prior_events: &EventBook,
    received_events: &EventBook,
    expected: u32,
) -> Result<CommutativeMergeResult, Status> {
    // Build EventBook with events up to `expected` sequence
    let events_at_expected = build_events_up_to_sequence(prior_events, expected);

    // Get state at expected sequence (what command assumed)
    let state_at_expected = business.replay(&events_at_expected).await?;

    // Get state at actual sequence (current reality before command)
    let state_at_actual = business.replay(prior_events).await?;

    // Build combined events: prior + command's new events
    let events_after_command = build_combined_events(prior_events, received_events);

    // Get state after applying command's events
    let state_after_command = business.replay(&events_after_command).await?;

    // Diff states to find fields changed by intervening events
    let intervening_changed = diff_state_fields(&state_at_expected, &state_at_actual);

    // Diff states to find fields changed by command
    let command_changed = diff_state_fields(&state_at_actual, &state_after_command);

    // Check if intervening changes and command changes are disjoint
    // Wildcard "*" means all fields → always overlaps (type change, decode failure, etc.)
    let has_overlap = if intervening_changed.contains("*") || command_changed.contains("*") {
        true
    } else {
        !intervening_changed.is_disjoint(&command_changed)
    };

    if has_overlap {
        tracing::debug!(
            intervening_fields = ?intervening_changed,
            command_fields = ?command_changed,
            "COMMUTATIVE: field overlap detected"
        );
        Ok(CommutativeMergeResult::Overlap)
    } else {
        tracing::debug!(
            intervening_fields = ?intervening_changed,
            command_fields = ?command_changed,
            "COMMUTATIVE: fields are disjoint"
        );
        Ok(CommutativeMergeResult::Disjoint)
    }
}

/// Build combined EventBook: prior events + new events from command response.
fn build_combined_events(prior_events: &EventBook, received_events: &EventBook) -> EventBook {
    let mut combined_pages = prior_events.pages.clone();
    combined_pages.extend(received_events.pages.iter().cloned());

    EventBook {
        cover: prior_events.cover.clone(),
        pages: combined_pages,
        snapshot: received_events.snapshot.clone(), // Use new snapshot if provided
        next_sequence: received_events.next_sequence,
    }
}

/// Build an EventBook with events up to a specific sequence (exclusive).
fn build_events_up_to_sequence(events: &EventBook, up_to_sequence: u32) -> EventBook {
    let filtered_pages: Vec<_> = events
        .pages
        .iter()
        .filter(|page| page.sequence_num() < up_to_sequence)
        .cloned()
        .collect();

    EventBook {
        cover: events.cover.clone(),
        pages: filtered_pages,
        snapshot: events.snapshot.clone(),
        next_sequence: up_to_sequence,
    }
}

/// Diff two Any-packed state messages to find changed field names.
///
/// # Fallback Strategy
///
/// This function uses a layered approach, trying more precise methods first:
///
/// 1. **Type URL check**: If types differ, return "*" (all fields). Different
///    state types mean a schema change occurred — we can't meaningfully compare.
///
/// 2. **Test state parsing**: For `test.StatefulState`, parse as JSON and compare
///    field-by-field. This supports framework testing without proto reflection.
///
/// 3. **Proto reflection**: If initialized, use `proto_reflect::diff_fields` for
///    proper protobuf field comparison. This handles production aggregates.
///
/// 4. **Byte comparison fallback**: If all else fails, compare raw bytes. If bytes
///    differ, assume all fields changed ("*"). This is maximally conservative.
///
/// # Why "*" When Types Differ
///
/// If `before.type_url != after.type_url`, the aggregate's state schema changed
/// (via upcasting, migration, or bug). Field-level comparison is meaningless
/// because field semantics may have changed. Treating this as "all fields changed"
/// forces a retry with fresh state, which is the safe choice.
fn diff_state_fields(
    before: &prost_types::Any,
    after: &prost_types::Any,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    // If types differ, assume complete overlap (all fields changed)
    if before.type_url != after.type_url {
        return ["*".to_string()].into_iter().collect();
    }

    // For test.StatefulState, parse as JSON-like and compare
    if before.type_url == "test.StatefulState" {
        return diff_test_state_fields(&before.value, &after.value);
    }

    // Try proto_reflect if pool is initialized
    if crate::proto_reflect::is_initialized() {
        match crate::proto_reflect::diff_fields(before, after) {
            Ok(fields) => return fields,
            Err(e) => {
                tracing::debug!(error = %e, "proto_reflect diff failed, using fallback");
            }
        }
    }

    // Fallback: if bytes are different, assume all fields changed
    if before.value != after.value {
        ["*".to_string()].into_iter().collect()
    } else {
        HashSet::new()
    }
}

/// Diff test state fields using simple JSON-like parsing.
///
/// Parses format: `{"field_a":100,"field_b":"hello"}`
fn diff_test_state_fields(before: &[u8], after: &[u8]) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    let before_str = String::from_utf8_lossy(before);
    let after_str = String::from_utf8_lossy(after);

    let before_fields = parse_test_state_fields(&before_str);
    let after_fields = parse_test_state_fields(&after_str);

    let mut changed = HashSet::new();

    // Find fields that differ
    for (key, before_val) in &before_fields {
        match after_fields.get(key) {
            Some(after_val) if after_val != before_val => {
                changed.insert(key.clone());
            }
            None => {
                changed.insert(key.clone());
            }
            _ => {}
        }
    }

    // Find fields only in after
    for key in after_fields.keys() {
        if !before_fields.contains_key(key) {
            changed.insert(key.clone());
        }
    }

    changed
}

/// Parse test state JSON-like format into field -> value map.
fn parse_test_state_fields(s: &str) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;

    let mut fields = HashMap::new();

    // Simple parsing of {"field_a":100,"field_b":"hello"}
    let s = s.trim_start_matches('{').trim_end_matches('}');
    for part in s.split(',') {
        if let Some((key, val)) = part.split_once(':') {
            let key = key.trim().trim_matches('"');
            let val = val.trim();
            fields.insert(key.to_string(), val.to_string());
        }
    }

    fields
}

/// Execute the aggregate command pipeline.
///
/// Flow:
/// - **Execute**: parse → extract edition → correlation_id → pre-validate → load →
///   transform → validate sequence → invoke → persist → post-persist → response
/// - **Speculative**: parse → extract edition → load temporal → transform → invoke →
///   response (no persist)
pub async fn execute_command_pipeline(
    ctx: &dyn AggregateContext,
    business: &dyn ClientLogic,
    command_book: CommandBook,
    mode: PipelineMode,
) -> Result<CommandResponse, Status> {
    match mode {
        PipelineMode::Execute => execute_mode(ctx, business, command_book).await,
        PipelineMode::Speculative {
            as_of_sequence,
            as_of_timestamp,
        } => {
            let temporal = match (as_of_sequence, as_of_timestamp) {
                (Some(seq), _) => TemporalQuery::AsOfSequence(seq),
                (_, Some(ts)) => TemporalQuery::AsOfTimestamp(ts),
                (None, None) => {
                    return Err(Status::invalid_argument(
                        crate::orchestration::errmsg::SPECULATIVE_REQUIRES_TEMPORAL,
                    ));
                }
            };
            speculative_mode(ctx, business, command_book, temporal).await
        }
    }
}

/// State for a retryable aggregate command operation.
struct AggregateOperation<'a> {
    ctx: &'a dyn AggregateContext,
    business: &'a dyn ClientLogic,
    command_book: CommandBook,
}

#[async_trait]
impl<'a> RetryableOperation for AggregateOperation<'a> {
    type Success = CommandResponse;
    type Failure = Status;

    fn name(&self) -> &str {
        "aggregate_command"
    }

    async fn try_execute(&mut self) -> RetryOutcome<Self::Success, Self::Failure> {
        match execute_mode(self.ctx, self.business, self.command_book.clone()).await {
            Ok(response) => RetryOutcome::Success(response),
            Err(status) => {
                if is_retryable_status(&status) {
                    RetryOutcome::Retryable(status)
                } else {
                    RetryOutcome::Fatal(status)
                }
            }
        }
    }
}

/// Execute the aggregate command pipeline with retry on sequence conflicts.
pub async fn execute_command_with_retry(
    ctx: &dyn AggregateContext,
    business: &dyn ClientLogic,
    command_book: CommandBook,
    backoff: ExponentialBuilder,
) -> Result<CommandResponse, Status> {
    let operation = AggregateOperation {
        ctx,
        business,
        command_book,
    };
    run_with_retry(operation, backoff).await
}

#[tracing::instrument(
    name = "aggregate.execute",
    skip_all,
    fields(domain, edition, root_uuid, merge_strategy)
)]
async fn execute_mode(
    ctx: &dyn AggregateContext,
    business: &dyn ClientLogic,
    mut command_book: CommandBook,
) -> Result<CommandResponse, Status> {
    let (domain, root_uuid) = parse_command_cover(&command_book)?;
    let edition = extract_edition(&command_book)?;
    let correlation_id = crate::orchestration::correlation::extract_correlation_id(&command_book)?;
    let merge_strategy = command_book.merge_strategy();

    let span = tracing::Span::current();
    span.record("domain", domain.as_str());
    span.record("edition", edition.as_str());
    span.record("root_uuid", tracing::field::display(&root_uuid));
    span.record("merge_strategy", tracing::field::debug(&merge_strategy));

    // Check for deferred sequences (saga-produced commands)
    let is_deferred = has_deferred_sequence(&command_book);

    // For angzarr_deferred, check idempotency first
    if let Some(deferred) = extract_angzarr_deferred(&command_book) {
        if let Some(existing_events) = ctx
            .check_deferred_idempotency(&domain, &edition, root_uuid, deferred)
            .await?
        {
            tracing::debug!(
                source_domain = deferred.source.as_ref().map(|c| c.domain.as_str()),
                source_seq = deferred.source_seq,
                "Deferred command already processed, returning cached result"
            );
            return Ok(CommandResponse {
                events: Some(existing_events),
                ..Default::default()
            });
        }
    }

    let expected = extract_command_sequence(&command_book);

    // For AGGREGATE_HANDLES, skip all coordinator-level sequence validation.
    // The aggregate is responsible for its own concurrency control.
    // For deferred sequences, we also skip pre-validation (we'll stamp after loading).
    if merge_strategy != MergeStrategy::MergeAggregateHandles && !is_deferred {
        // Pre-validate sequence (gRPC fast-path, no-op for local)
        ctx.pre_validate_sequence(&domain, &edition, root_uuid, expected)
            .await?;
    }

    // Load prior events
    let prior_events = ctx
        .load_prior_events(&domain, &edition, root_uuid, &TemporalQuery::Current)
        .await?;

    // Transform events (upcasting)
    let prior_events = ctx.transform_events(&domain, prior_events).await?;

    // Get actual sequence
    let actual = prior_events.next_sequence();

    // For deferred sequences, stamp actual sequence onto command pages
    if is_deferred {
        stamp_deferred_sequences(&mut command_book, actual);
        tracing::debug!(
            actual,
            "Stamped deferred sequence with actual sequence number"
        );
    }

    // Sequence validation based on merge strategy (skip for deferred)
    let sequence_mismatch = !is_deferred && expected != actual;

    // Track if we need post-execution commutative check
    let needs_commutative_check =
        sequence_mismatch && merge_strategy == MergeStrategy::MergeCommutative;

    if sequence_mismatch {
        match merge_strategy {
            MergeStrategy::MergeStrict => {
                // STRICT: Return FAILED_PRECONDITION (retryable) for update-and-retry flow.
                // The retry loop will reload fresh state and retry the command.
                return Err(Status::failed_precondition(format!(
                    "{}{expected}, aggregate at {actual}",
                    crate::orchestration::errmsg::SEQUENCE_MISMATCH
                )));
            }
            MergeStrategy::MergeCommutative => {
                // COMMUTATIVE: Proceed to execution, check field overlap afterward.
                // We'll verify after command execution whether the changes are disjoint.
                tracing::debug!(
                    expected,
                    actual,
                    "COMMUTATIVE: sequence mismatch, will check field overlap post-execution"
                );
            }
            MergeStrategy::MergeManual => {
                // MANUAL: Send to DLQ for human review, return ABORTED (non-retryable).
                ctx.send_to_dlq(&command_book, expected, actual, &domain)
                    .await;
                return Err(Status::aborted(format!(
                    "{}{expected}, aggregate at {actual}{}",
                    crate::orchestration::errmsg::SEQUENCE_MISMATCH,
                    crate::orchestration::errmsg::SEQUENCE_MISMATCH_DLQ_SUFFIX
                )));
            }
            MergeStrategy::MergeAggregateHandles => {
                // No validation - aggregate handles it
            }
        }
    }

    // Invoke client logic
    let contextual_command = ContextualCommand {
        events: Some(prior_events.clone()),
        command: Some(command_book),
    };

    let response = business.invoke(contextual_command).await.map_err(|e| {
        tracing::error!(error = %e, "client logic invocation failed");
        e
    })?;
    let received_events = extract_events_from_response(response, correlation_id.to_string())?;

    // Post-execution commutative check: verify field overlap after we know what changed
    if needs_commutative_check {
        match check_commutative_overlap(business, &prior_events, &received_events, expected).await {
            Ok(CommutativeMergeResult::Disjoint) => {
                tracing::debug!(
                    expected,
                    actual,
                    "COMMUTATIVE: disjoint fields confirmed, proceeding to persist"
                );
            }
            Ok(CommutativeMergeResult::Overlap) => {
                // Fields overlap - discard result and retry
                return Err(Status::failed_precondition(format!(
                    "{}{expected}, aggregate at {actual}",
                    crate::orchestration::errmsg::SEQUENCE_MISMATCH_OVERLAP
                )));
            }
            Err(e) => {
                // Replay unavailable or error - degrade to STRICT behavior
                tracing::debug!(
                    expected,
                    actual,
                    error = %e,
                    "COMMUTATIVE: degrading to STRICT due to Replay failure"
                );
                return Err(Status::failed_precondition(format!(
                    "{}{expected}, aggregate at {actual}",
                    crate::orchestration::errmsg::SEQUENCE_MISMATCH
                )));
            }
        }
    }

    // Persist (compares prior with received to detect new events/snapshot)
    let mut persisted = ctx
        .persist_events(
            &prior_events,
            &received_events,
            &domain,
            &edition,
            root_uuid,
            &correlation_id,
        )
        .await?;

    // Set next_sequence on persisted EventBook for callers
    calculate_set_next_seq(&mut persisted);

    // Post-persist: publish + sync projectors
    let projections = ctx.post_persist(&persisted).await?;

    Ok(CommandResponse {
        events: Some(persisted),
        projections,
    })
}

#[tracing::instrument(name = "aggregate.speculative", skip_all, fields(domain, edition, root_uuid, ?temporal))]
async fn speculative_mode(
    ctx: &dyn AggregateContext,
    business: &dyn ClientLogic,
    command_book: CommandBook,
    temporal: TemporalQuery,
) -> Result<CommandResponse, Status> {
    let (domain, root_uuid) = parse_command_cover(&command_book)?;
    let edition = extract_edition(&command_book)?;

    let span = tracing::Span::current();
    span.record("domain", domain.as_str());
    span.record("edition", edition.as_str());
    span.record("root_uuid", tracing::field::display(&root_uuid));

    let prior_events = ctx
        .load_prior_events(&domain, &edition, root_uuid, &temporal)
        .await?;
    let prior_events = ctx.transform_events(&domain, prior_events).await?;

    let contextual_command = ContextualCommand {
        events: Some(prior_events),
        command: Some(command_book),
    };

    let response = business.invoke(contextual_command).await.map_err(|e| {
        tracing::error!(error = %e, "client logic invocation failed");
        e
    })?;

    // For speculative mode, extract events but don't set correlation_id
    let speculative_events = extract_events_from_response(response, String::new())?;

    Ok(CommandResponse {
        events: Some(speculative_events),
        projections: vec![],
    })
}

/// Response from fact injection.
#[derive(Debug, Clone)]
pub struct FactResponse {
    /// The persisted events (with real sequence numbers).
    pub events: EventBook,
    /// Projections from sync projectors.
    pub projections: Vec<Projection>,
    /// True if this was a duplicate request (external_id already processed).
    pub already_processed: bool,
}

/// Parse domain and root UUID from an EventBook cover.
pub fn parse_event_cover(event_book: &EventBook) -> Result<(String, Uuid), Status> {
    let cover = event_book.cover.as_ref().ok_or_else(|| {
        Status::invalid_argument(crate::orchestration::errmsg::EVENT_BOOK_MISSING_COVER)
    })?;

    let domain = cover.domain.clone();
    crate::validation::validate_domain(&domain)?;

    let root = cover.root.as_ref().ok_or_else(|| {
        Status::invalid_argument(crate::orchestration::errmsg::COVER_MISSING_ROOT)
    })?;

    let root_uuid = Uuid::from_slice(&root.value).map_err(|e| {
        Status::invalid_argument(format!("{}{e}", crate::orchestration::errmsg::INVALID_UUID))
    })?;

    Ok((domain, root_uuid))
}

/// Extract edition from an EventBook's Cover.
fn extract_event_edition(event_book: &EventBook) -> Result<String, Status> {
    let edition = event_book.edition().to_string();
    crate::validation::validate_edition(&edition)?;
    Ok(edition)
}

/// Execute the fact injection pipeline.
///
/// Fact events are external realities that cannot be rejected. The pipeline:
/// 1. Validates Cover and extracts identifiers
/// 2. Checks idempotency via `PageHeader.external_deferred.external_id`
/// 3. Loads prior events for aggregate state
/// 4. Optionally routes to aggregate for state update
/// 5. Assigns real sequence numbers (replacing ExternalDeferredSequence markers)
/// 6. Persists and publishes events
///
/// # Arguments
///
/// * `ctx` - Aggregate context for storage access
/// * `business` - Optional client logic for state update (None = direct persist)
/// * `fact_events` - EventBook containing fact events with ExternalDeferredSequence markers
///
/// # Returns
///
/// The persisted events with real sequence numbers.
#[tracing::instrument(
    name = "aggregate.fact_inject",
    skip_all,
    fields(domain, edition, root_uuid, external_id)
)]
pub async fn execute_fact_pipeline(
    ctx: &dyn AggregateContext,
    business: Option<&dyn ClientLogic>,
    fact_events: EventBook,
) -> Result<FactResponse, Status> {
    use crate::proto::page_header::SequenceType;

    let (domain, root_uuid) = parse_event_cover(&fact_events)?;
    let edition = extract_event_edition(&fact_events)?;
    let correlation_id = fact_events.correlation_id().to_string();

    // Extract external_id from first page's header if it has external_deferred
    let external_id = fact_events
        .pages
        .first()
        .and_then(|p| p.header.as_ref())
        .and_then(|h| match &h.sequence_type {
            Some(SequenceType::ExternalDeferred(ext)) => Some(ext.external_id.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let span = tracing::Span::current();
    span.record("domain", domain.as_str());
    span.record("edition", edition.as_str());
    span.record("root_uuid", tracing::field::display(&root_uuid));
    span.record("external_id", external_id.as_str());

    // Check idempotency if external_id is provided
    if !external_id.is_empty() {
        if let Some((first_seq, last_seq)) = ctx
            .check_fact_idempotency(&domain, &edition, root_uuid, &external_id)
            .await?
        {
            // Already processed - load and return existing events
            tracing::debug!(
                external_id = %external_id,
                first_seq = first_seq,
                last_seq = last_seq,
                "Fact already processed (idempotent response)"
            );

            let prior_events = ctx
                .load_prior_events(&domain, &edition, root_uuid, &TemporalQuery::Current)
                .await?;

            // Filter to only the events that were part of this fact injection
            let mut existing_events = prior_events.clone();
            existing_events.pages = prior_events
                .pages
                .into_iter()
                .filter(|p| {
                    if let Some(SequenceType::Sequence(seq)) =
                        p.header.as_ref().and_then(|h| h.sequence_type.as_ref())
                    {
                        *seq >= first_seq && *seq <= last_seq
                    } else {
                        false
                    }
                })
                .collect();

            return Ok(FactResponse {
                events: existing_events,
                projections: vec![],
                already_processed: true,
            });
        }
    }

    // Validate that at least one page has ExternalDeferred (fact marker)
    let has_fact_marker = fact_events.pages.iter().any(|p| {
        matches!(
            p.header.as_ref().and_then(|h| h.sequence_type.as_ref()),
            Some(SequenceType::ExternalDeferred(_))
        )
    });
    if !has_fact_marker {
        return Err(Status::invalid_argument(
            crate::orchestration::errmsg::FACT_EVENTS_MISSING_MARKER,
        ));
    }

    // Load prior events to determine next sequence
    let prior_events = ctx
        .load_prior_events(&domain, &edition, root_uuid, &TemporalQuery::Current)
        .await?;

    // Transform events (upcasting)
    let prior_events = ctx.transform_events(&domain, prior_events).await?;

    // Get next available sequence
    let next_seq = prior_events.next_sequence();

    // Optionally invoke client logic to update aggregate state
    let processed_events = if let Some(logic) = business {
        let fact_ctx = FactContext {
            facts: fact_events.clone(),
            prior_events: Some(prior_events.clone()),
        };
        logic.invoke_fact(fact_ctx).await?
    } else {
        fact_events.clone()
    };

    // Assign real sequence numbers, replacing ExternalDeferredSequence markers.
    // Timestamps default to "now" if not provided by the external source.
    //
    // NOTE: External facts from historical systems (replays, migrations) should provide
    // their original timestamps. If `created_at` is missing, we default to current time
    // which may misrepresent when the fact actually occurred. Temporal queries against
    // such facts would return incorrect results. Callers injecting historical facts
    // should always include the original timestamp.
    let mut final_pages = Vec::with_capacity(processed_events.pages.len());
    let mut current_seq = next_seq;

    for page in processed_events.pages {
        let new_page = crate::proto::EventPage {
            header: Some(crate::proto::PageHeader {
                sequence_type: Some(SequenceType::Sequence(current_seq)),
            }),
            created_at: page
                .created_at
                .or_else(|| Some(prost_types::Timestamp::from(std::time::SystemTime::now()))),
            payload: page.payload,
        };
        final_pages.push(new_page);
        current_seq += 1;
    }

    let events_to_persist = EventBook {
        cover: fact_events.cover.clone(),
        pages: final_pages,
        snapshot: processed_events.snapshot,
        next_sequence: current_seq,
    };

    // Persist events
    let mut persisted = ctx
        .persist_events(
            &prior_events,
            &events_to_persist,
            &domain,
            &edition,
            root_uuid,
            &correlation_id,
        )
        .await?;

    // Set next_sequence
    calculate_set_next_seq(&mut persisted);

    // Record idempotency if external_id is provided
    if !external_id.is_empty() && !persisted.pages.is_empty() {
        let first_seq = persisted
            .pages
            .first()
            .and_then(|p| {
                if let Some(SequenceType::Sequence(s)) =
                    p.header.as_ref().and_then(|h| h.sequence_type.as_ref())
                {
                    Some(*s)
                } else {
                    None
                }
            })
            .unwrap_or(next_seq);
        let last_seq = persisted
            .pages
            .last()
            .and_then(|p| {
                if let Some(SequenceType::Sequence(s)) =
                    p.header.as_ref().and_then(|h| h.sequence_type.as_ref())
                {
                    Some(*s)
                } else {
                    None
                }
            })
            .unwrap_or(current_seq.saturating_sub(1));

        ctx.record_fact_idempotency(
            &domain,
            &edition,
            root_uuid,
            &external_id,
            first_seq,
            last_seq,
        )
        .await?;
    }

    // Post-persist: publish + sync projectors
    let projections = ctx.post_persist(&persisted).await?;

    Ok(FactResponse {
        events: persisted,
        projections,
        already_processed: false,
    })
}

#[cfg(test)]
mod tests;
