//! Mock storage implementations for testing.

mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::MockEventStore;
pub use position_store::MockPositionStore;
pub use snapshot_store::MockSnapshotStore;

#[cfg(test)]
mod tests;
