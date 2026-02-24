//! Shared storage contract test suites.
//!
//! Defines reusable test suites for EventStore, SnapshotStore, and PositionStore
//! trait contracts. Each backend test module imports and runs these via macros.

pub mod event_store_tests;
pub mod position_store_tests;
pub mod snapshot_store_tests;
