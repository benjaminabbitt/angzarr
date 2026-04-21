//! Tests for the sync-mode policy predicate.
//!
//! Each variant of `SyncMode` has a documented effect on post-persist projector
//! invocation. These tests pin that mapping so a future addition or rename
//! can't silently change behavior — the proto enum is shared with non-Rust
//! clients, so a drift here would cross language boundaries.

use super::should_call_sync_projectors;
use crate::proto::SyncMode;

/// SIMPLE means "sync projectors only, no saga cascade" — projectors must run
/// synchronously so the caller observes the projection state on return.
#[test]
fn test_simple_waits_for_projectors() {
    assert!(should_call_sync_projectors(Some(SyncMode::Simple)));
}

/// CASCADE includes projectors plus saga/PM fan-out — both blocking.
#[test]
fn test_cascade_waits_for_projectors() {
    assert!(should_call_sync_projectors(Some(SyncMode::Cascade)));
}

/// ASYNC is fire-and-forget; the caller does not wait for projectors.
#[test]
fn test_async_skips_projectors() {
    assert!(!should_call_sync_projectors(Some(SyncMode::Async)));
}

/// DECISION returns after the aggregate's accept/reject decision; projectors
/// propagate asynchronously, the same as ASYNC. This is the rule the
/// SYNC_MODE_DECISION proto variant introduced — without skipping, a process
/// manager waiting on DECISION would also block on every downstream projector.
#[test]
fn test_decision_skips_projectors() {
    assert!(!should_call_sync_projectors(Some(SyncMode::Decision)));
}

/// `None` means no sync mode was set on the context (e.g., bus-driven event
/// handlers that construct contexts without a request envelope). Default to
/// skip; making it block would silently change non-request-driven flows.
#[test]
fn test_none_skips_projectors() {
    assert!(!should_call_sync_projectors(None));
}
