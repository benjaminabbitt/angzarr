//! Redis storage implementations.

mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::RedisEventStore;
pub use position_store::RedisPositionStore;
pub use snapshot_store::RedisSnapshotStore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::EventStore;
    use uuid::Uuid;

    // Integration tests require Redis running
    // Run with: cargo test --features redis -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_redis_event_store() {
        let store = RedisEventStore::new("redis://localhost:6379", Some("test"))
            .await
            .expect("Failed to connect to Redis");

        let domain = "test-domain";
        let root = Uuid::new_v4();

        // Create test events
        let events = vec![
            crate::proto::EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: None,
                created_at: None,
            },
            crate::proto::EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(1)),
                event: None,
                created_at: None,
            },
        ];

        // Add events
        store
            .add(domain, "angzarr", root, events.clone(), "")
            .await
            .expect("Failed to add events");

        // Retrieve events
        let retrieved = store
            .get(domain, "angzarr", root)
            .await
            .expect("Failed to get events");
        assert_eq!(retrieved.len(), 2);

        // Check next sequence
        let next_seq = store
            .get_next_sequence(domain, "angzarr", root)
            .await
            .expect("Failed to get next sequence");
        assert_eq!(next_seq, 2);

        // List roots
        let roots = store
            .list_roots(domain, "angzarr")
            .await
            .expect("Failed to list roots");
        assert!(roots.contains(&root));
    }
}
