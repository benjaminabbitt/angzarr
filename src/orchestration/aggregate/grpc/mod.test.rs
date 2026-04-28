//! Tests for the gRPC aggregate context.

use super::should_skip_post_persist;
use crate::proto::SyncMode;

// `should_skip_post_persist` decides whether the post-persist callback
// short-circuits — true only for ISOLATED, false for every other mode.
// Pure decision function, fully unit-testable without standing up the
// full GrpcAggregateContext.

#[test]
fn skip_for_isolated_only() {
    assert!(should_skip_post_persist(Some(SyncMode::Isolated)));
}

#[test]
fn no_skip_for_async() {
    // ASYNC publishes to bus (downstream runs async). Distinct from
    // ISOLATED which suppresses the publish entirely.
    assert!(!should_skip_post_persist(Some(SyncMode::Async)));
}

#[test]
fn no_skip_for_simple() {
    // SIMPLE waits for sync projectors but events still reach the bus.
    assert!(!should_skip_post_persist(Some(SyncMode::Simple)));
}

#[test]
fn no_skip_for_cascade() {
    // CASCADE runs the full sync chain; events also reach the bus.
    assert!(!should_skip_post_persist(Some(SyncMode::Cascade)));
}

#[test]
fn no_skip_for_decision() {
    // DECISION returns the accept/reject synchronously but lets
    // projectors + sagas run async via the bus. Crucially NOT the
    // same as ISOLATED — DECISION still publishes.
    assert!(!should_skip_post_persist(Some(SyncMode::Decision)));
}

#[test]
fn no_skip_for_none() {
    // Missing sync_mode (None) defaults to async behavior — bus
    // publish proceeds.
    assert!(!should_skip_post_persist(None));
}
