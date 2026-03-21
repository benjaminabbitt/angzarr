//! Commutative merge logic for concurrent write detection.
//!
//! Implements field-level conflict detection for COMMUTATIVE merge strategy.
//! When concurrent writes touch disjoint fields, they can safely proceed
//! without retry.

use std::collections::HashSet;

use tonic::Status;

use crate::proto::EventBook;
use crate::proto_ext::EventPageExt;

use super::traits::ClientLogic;

#[cfg(any(test, feature = "test-utils"))]
#[path = "merge_test_support.rs"]
pub(crate) mod test_support;

/// Result of commutative merge check.
#[derive(Debug)]
pub(crate) enum CommutativeMergeResult {
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
pub(crate) async fn check_commutative_overlap(
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
pub(crate) fn build_combined_events(
    prior_events: &EventBook,
    received_events: &EventBook,
) -> EventBook {
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
pub(crate) fn build_events_up_to_sequence(events: &EventBook, up_to_sequence: u32) -> EventBook {
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
/// 2. **Test state handler**: In test builds, handles `test.StatefulState` with
///    simple JSON-like parsing for field-level comparison.
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
pub(crate) fn diff_state_fields(
    before: &prost_types::Any,
    after: &prost_types::Any,
) -> HashSet<String> {
    // If types differ, assume complete overlap (all fields changed)
    if before.type_url != after.type_url {
        return ["*".to_string()].into_iter().collect();
    }

    // Test state handler for test.StatefulState type
    #[cfg(any(test, feature = "test-utils"))]
    if before.type_url == "test.StatefulState" {
        return test_support::diff_test_state_fields(&before.value, &after.value);
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

// ============================================================================
// Two-Phase Commit Conflict Detection
// ============================================================================

/// Result of cascade conflict check.
// TODO: Wire into pipeline when cascade mode goes live
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum CascadeConflictResult {
    /// No uncommitted events, or no field overlap - safe to proceed.
    NoConflict,
    /// Fields overlap with uncommitted events from other cascades.
    Conflict {
        cascade_ids: Vec<String>,
        overlapping_fields: HashSet<String>,
    },
}

/// Partition events by commit status.
///
/// Returns (committed_events, uncommitted_events).
#[allow(dead_code)]
pub(crate) fn partition_by_commit_status(
    events: &EventBook,
) -> (EventBook, Vec<&crate::proto::EventPage>) {
    let committed_pages: Vec<_> = events
        .pages
        .iter()
        .filter(|p| p.committed)
        .cloned()
        .collect();

    let uncommitted: Vec<_> = events.pages.iter().filter(|p| !p.committed).collect();

    let committed_book = EventBook {
        cover: events.cover.clone(),
        pages: committed_pages,
        snapshot: events.snapshot.clone(),
        next_sequence: events.next_sequence,
    };

    (committed_book, uncommitted)
}

/// Check for cascade conflict with uncommitted events.
///
/// # Algorithm
///
/// 1. Partition prior events into committed and uncommitted
/// 2. If no uncommitted events, no conflict possible
/// 3. Compute "locked" fields: diff between committed-only state and all state
/// 4. Compute command's fields: diff between current state and after-command state
/// 5. Check for overlap between locked and command fields
///
/// This implements optimistic field-level locking: uncommitted events "lock"
/// the fields they touched. New commands can proceed if they don't touch those fields.
#[allow(dead_code)]
pub(crate) async fn check_cascade_conflict(
    business: &dyn ClientLogic,
    prior_events: &EventBook,
    command_events: &EventBook,
) -> Result<CascadeConflictResult, Status> {
    let (committed, uncommitted) = partition_by_commit_status(prior_events);

    // No uncommitted events = no conflict possible
    if uncommitted.is_empty() {
        return Ok(CascadeConflictResult::NoConflict);
    }

    // Compute locked fields: what uncommitted events changed
    let state_committed = business.replay(&committed).await?;
    let state_all = business.replay(prior_events).await?;
    let locked_fields = diff_state_fields(&state_committed, &state_all);

    // Compute fields this command would touch
    let combined = build_combined_events(prior_events, command_events);
    let state_after_cmd = business.replay(&combined).await?;
    let command_fields = diff_state_fields(&state_all, &state_after_cmd);

    // Wildcard means all fields - always conflicts
    if locked_fields.contains("*") || command_fields.contains("*") {
        let cascade_ids: Vec<_> = uncommitted
            .iter()
            .filter_map(|e| e.cascade_id.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        return Ok(CascadeConflictResult::Conflict {
            cascade_ids,
            overlapping_fields: command_fields,
        });
    }

    // Check for field overlap
    let overlap: HashSet<_> = locked_fields
        .intersection(&command_fields)
        .cloned()
        .collect();

    if !overlap.is_empty() {
        let cascade_ids: Vec<_> = uncommitted
            .iter()
            .filter_map(|e| e.cascade_id.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        return Ok(CascadeConflictResult::Conflict {
            cascade_ids,
            overlapping_fields: overlap,
        });
    }

    Ok(CascadeConflictResult::NoConflict)
}
