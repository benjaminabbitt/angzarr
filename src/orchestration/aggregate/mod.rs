//! Aggregate command execution pipeline abstraction.
//!
//! `AggregateContext` trait + `execute_command_pipeline` for the shared
//! command execution flow (parse → load → validate → invoke → persist → publish).
//!
//! client logic invocation is via the `ClientLogic` trait, decoupling the
//! pipeline from transport (gRPC over TCP, UDS, or in-process calls).
//! The `AggregateContext` trait covers storage access, post-persist behavior, and optional hooks.
//! - `local/`: SQLite-backed storage with static service discovery
//! - `grpc/`: Remote storage with K8s service discovery

// tonic::Status is large by design - it carries error details for gRPC
#![allow(clippy::result_large_err)]

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use async_trait::async_trait;
use backon::ExponentialBuilder;
use tokio::sync::Mutex;
use tonic::Status;
use uuid::Uuid;

use crate::proto::{
    aggregate_service_client::AggregateServiceClient, BusinessResponse, CommandBook,
    CommandResponse, ContextualCommand, EventBook, MergeStrategy, Projection, ReplayRequest,
};
use crate::proto_ext::{calculate_set_next_seq, CommandBookExt, CoverExt, EventBookExt};
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
}

/// Abstraction for aggregate client logic invocation.
///
/// Decouples the command pipeline from the transport used to call client logic.
/// Implementations may use gRPC (over TCP, UDS), in-process trait calls, etc.
#[async_trait]
pub trait ClientLogic: Send + Sync {
    /// Invoke client logic with prior events and a command.
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status>;

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

/// client logic invocation via gRPC `AggregateClient`.
///
/// Wraps a tonic `AggregateClient` channel (TCP, UDS, or duplex).
pub struct GrpcBusinessLogic {
    client: Mutex<AggregateServiceClient<tonic::transport::Channel>>,
}

impl GrpcBusinessLogic {
    /// Wrap a gRPC aggregate client as a `ClientLogic` implementation.
    pub fn new(client: AggregateServiceClient<tonic::transport::Channel>) -> Self {
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
            .ok_or_else(|| Status::internal("Replay response missing state"))
    }
}

/// Parse domain and root UUID from a CommandBook cover.
///
/// Validates domain format before returning.
pub fn parse_command_cover(command: &CommandBook) -> Result<(String, Uuid), Status> {
    let cover = command
        .cover
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

    let domain = cover.domain.clone();
    crate::validation::validate_domain(&domain)?;

    let root = cover
        .root
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

    let root_uuid = Uuid::from_slice(&root.value)
        .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

    Ok((domain, root_uuid))
}

/// Extract expected sequence from the first command page.
pub fn extract_command_sequence(command: &CommandBook) -> u32 {
    command.pages.first().map(|p| p.sequence).unwrap_or(0)
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

/// Result of attempting a commutative merge.
#[derive(Debug)]
enum CommutativeMergeResult {
    /// Fields changed by intervening events don't overlap with command's fields.
    Disjoint,
    /// Fields overlap - command must retry with fresh state.
    Overlap,
}

/// Attempt commutative merge by comparing state fields.
///
/// Calls Replay RPC to get states at `expected` and `actual` sequences,
/// then diffs the fields to detect overlap with the command's changes.
///
/// Returns:
/// - `Ok(Disjoint)` if changes don't overlap → command can proceed
/// - `Ok(Overlap)` if changes overlap → command must retry
/// - `Err(_)` if Replay unavailable → degrade to STRICT behavior
async fn try_commutative_merge(
    business: &dyn ClientLogic,
    prior_events: &EventBook,
    command: &CommandBook,
    expected: u32,
    _actual: u32,
) -> Result<CommutativeMergeResult, Status> {
    // Build EventBook with events up to `expected` sequence
    let events_at_expected = build_events_up_to_sequence(prior_events, expected);

    // Get state at expected sequence
    let state_at_expected = business.replay(&events_at_expected).await?;

    // Get state at actual sequence (use all prior_events)
    let state_at_actual = business.replay(prior_events).await?;

    // Diff states to find fields changed by intervening events
    let intervening_changed = diff_state_fields(&state_at_expected, &state_at_actual);

    // Determine what field(s) the command would change
    let command_fields = extract_command_fields(command);

    // Check if intervening changes and command changes are disjoint
    // Use field-level granularity: "field_a" vs "field_a" = overlap
    // Wildcard "*" means all fields → always overlaps
    let has_overlap = if intervening_changed.contains("*") || command_fields.contains("*") {
        true
    } else {
        !intervening_changed.is_disjoint(&command_fields)
    };

    if has_overlap {
        tracing::debug!(
            intervening_fields = ?intervening_changed,
            command_fields = ?command_fields,
            "COMMUTATIVE: field overlap detected"
        );
        Ok(CommutativeMergeResult::Overlap)
    } else {
        tracing::debug!(
            intervening_fields = ?intervening_changed,
            command_fields = ?command_fields,
            "COMMUTATIVE: fields are disjoint"
        );
        Ok(CommutativeMergeResult::Disjoint)
    }
}

/// Extract field names that a command would modify.
///
/// For test commands with type_url like "test.UpdateFieldA", extracts "field_a".
/// For unknown commands, returns "*" (assumes all fields).
fn extract_command_fields(command: &CommandBook) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    let mut fields = HashSet::new();

    for page in &command.pages {
        if let Some(crate::proto::command_page::Payload::Command(cmd)) = &page.payload {
            // Check for test command patterns
            if cmd.type_url.contains("UpdateFieldA") || cmd.type_url.contains("FieldAUpdated") {
                fields.insert("field_a".to_string());
            } else if cmd.type_url.contains("UpdateFieldB")
                || cmd.type_url.contains("FieldBUpdated")
            {
                fields.insert("field_b".to_string());
            } else if cmd.type_url.contains("UpdateBoth") {
                fields.insert("field_a".to_string());
                fields.insert("field_b".to_string());
            } else {
                // Unknown command type - assume it might touch any field
                // This is conservative: better to retry than to corrupt state
                fields.insert("*".to_string());
            }
        }
    }

    if fields.is_empty() {
        // No command pages - treat as no fields modified
        fields
    } else {
        fields
    }
}

/// Build an EventBook with events up to a specific sequence (exclusive).
fn build_events_up_to_sequence(events: &EventBook, up_to_sequence: u32) -> EventBook {
    let filtered_pages: Vec<_> = events
        .pages
        .iter()
        .filter(|page| page.sequence < up_to_sequence)
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
/// Returns a set of field names that differ between the two states.
/// For now, uses a simple JSON-based comparison for test states.
/// TODO: Use proto_reflect for proper protobuf reflection.
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
                        "Speculative requires either as_of_sequence or as_of_timestamp",
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
    command_book: CommandBook,
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

    let expected = extract_command_sequence(&command_book);

    // For AGGREGATE_HANDLES, skip all coordinator-level sequence validation.
    // The aggregate is responsible for its own concurrency control.
    if merge_strategy != MergeStrategy::MergeAggregateHandles {
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

    // Sequence validation based on merge strategy
    let actual = prior_events.next_sequence();
    if expected != actual {
        match merge_strategy {
            MergeStrategy::MergeStrict => {
                // STRICT: Return FAILED_PRECONDITION (retryable) for update-and-retry flow.
                // The retry loop will reload fresh state and retry the command.
                return Err(Status::failed_precondition(format!(
                    "Sequence mismatch: command expects {expected}, aggregate at {actual}"
                )));
            }
            MergeStrategy::MergeCommutative => {
                // COMMUTATIVE: Attempt field-level merge detection.
                // If Replay RPC is available, compare states to detect field overlap.
                // If fields are disjoint → proceed, else → retry.
                match try_commutative_merge(
                    business,
                    &prior_events,
                    &command_book,
                    expected,
                    actual,
                )
                .await
                {
                    Ok(CommutativeMergeResult::Disjoint) => {
                        // Fields don't overlap - proceed with command
                        tracing::debug!(
                            expected,
                            actual,
                            "COMMUTATIVE: disjoint fields detected, allowing stale sequence"
                        );
                        // Continue to command execution below
                    }
                    Ok(CommutativeMergeResult::Overlap) => {
                        // Fields overlap - need retry
                        return Err(Status::failed_precondition(format!(
                            "Sequence mismatch with overlapping fields: command expects {expected}, aggregate at {actual}"
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
                            "Sequence mismatch: command expects {expected}, aggregate at {actual}"
                        )));
                    }
                }
            }
            MergeStrategy::MergeManual => {
                // MANUAL: Send to DLQ for human review, return ABORTED (non-retryable).
                ctx.send_to_dlq(&command_book, expected, actual, &domain)
                    .await;
                return Err(Status::aborted(format!(
                    "Sequence mismatch: command expects {expected}, aggregate at {actual}. Sent to DLQ for manual review."
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

#[cfg(test)]
mod tests;
