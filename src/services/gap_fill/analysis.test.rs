//! Tests for gap analysis pure functions.
//!
//! Gap analysis determines whether an EventBook is complete relative to
//! a handler's checkpoint. This is the core decision logic - no I/O involved.

use super::*;

// ============================================================================
// analyze_gap() Tests
// ============================================================================

/// New aggregate: handler has never seen this (domain, root) before.
/// First event at seq 0 is expected - fetch full history from 0.
#[test]
fn test_analyze_gap_new_aggregate() {
    let result = analyze_gap(None, 0);
    assert_eq!(result, GapAnalysis::NewAggregate);
}

/// New aggregate with events starting mid-stream.
/// Handler has no checkpoint but receives events starting at 5 - need 0-4.
#[test]
fn test_analyze_gap_new_aggregate_mid_stream() {
    let result = analyze_gap(None, 5);
    assert_eq!(result, GapAnalysis::NewAggregate);
}

/// Complete: first event is exactly checkpoint + 1.
/// Handler saw up to 5, now receives event 6 - perfect continuity.
#[test]
fn test_analyze_gap_complete_exact() {
    let result = analyze_gap(Some(5), 6);
    assert_eq!(result, GapAnalysis::Complete);
}

/// Complete: first event is at or before checkpoint.
/// Handler saw up to 5, receives events starting at 3 - already processed, no gap.
/// (Handler will skip events <= checkpoint, but EventBook is "complete")
#[test]
fn test_analyze_gap_complete_already_processed() {
    let result = analyze_gap(Some(5), 3);
    assert_eq!(result, GapAnalysis::Complete);
}

/// Gap: first event is beyond checkpoint + 1.
/// Handler saw up to 5, receives events starting at 10 - missing 6-9.
#[test]
fn test_analyze_gap_has_gap() {
    let result = analyze_gap(Some(5), 10);
    assert_eq!(
        result,
        GapAnalysis::Gap {
            checkpoint: 5,
            first_event_seq: 10,
        }
    );
}

/// Edge case: checkpoint at 0, first event at 2.
/// Handler processed seq 0, receives event 2 - missing seq 1.
#[test]
fn test_analyze_gap_checkpoint_zero_with_gap() {
    let result = analyze_gap(Some(0), 2);
    assert_eq!(
        result,
        GapAnalysis::Gap {
            checkpoint: 0,
            first_event_seq: 2,
        }
    );
}

/// Edge case: checkpoint at 0, first event at 1.
/// Handler processed seq 0, receives event 1 - perfect continuity.
#[test]
fn test_analyze_gap_checkpoint_zero_complete() {
    let result = analyze_gap(Some(0), 1);
    assert_eq!(result, GapAnalysis::Complete);
}

/// Edge case: checkpoint at 0, first event at 0.
/// Handler processed seq 0, receives event 0 again - already seen, complete.
#[test]
fn test_analyze_gap_checkpoint_zero_replay() {
    let result = analyze_gap(Some(0), 0);
    assert_eq!(result, GapAnalysis::Complete);
}
