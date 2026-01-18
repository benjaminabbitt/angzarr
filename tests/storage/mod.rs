//! Shared storage integration tests.
//!
//! Tests the EventStore and SnapshotStore interfaces against all implementations.
//! Each implementation module imports these test functions and runs them.

pub mod event_store_tests;
pub mod snapshot_store_tests;
