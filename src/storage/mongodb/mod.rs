//! MongoDB implementations of storage interfaces.

mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::MongoEventStore;
pub use position_store::MongoPositionStore;
pub use snapshot_store::MongoSnapshotStore;

/// Collection names.
pub(crate) const EVENTS_COLLECTION: &str = "events";
pub(crate) const SNAPSHOTS_COLLECTION: &str = "snapshots";
pub(crate) const POSITIONS_COLLECTION: &str = "positions";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_names() {
        assert_eq!(EVENTS_COLLECTION, "events");
        assert_eq!(SNAPSHOTS_COLLECTION, "snapshots");
        assert_eq!(POSITIONS_COLLECTION, "positions");
    }
}
