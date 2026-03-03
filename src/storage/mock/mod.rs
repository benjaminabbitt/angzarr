//! Mock storage implementations for testing.

mod event_store;
mod snapshot_store;

pub use event_store::MockEventStore;
pub use snapshot_store::MockSnapshotStore;

#[cfg(test)]
mod tests;
