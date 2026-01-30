//! Snapshot store integration tests.

use crate::common::*;
use angzarr::proto::Snapshot;

#[tokio::test]
async fn test_snapshot_store_and_retrieve() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("counters", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let root = Uuid::new_v4();

    // Store snapshot directly (Snapshot has only sequence + state)
    let snapshot = Snapshot {
        sequence: 10,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"snapshot-data".to_vec(),
        }),
    };

    runtime
        .snapshot_store("counters")
        .unwrap()
        .put("counters", DEFAULT_EDITION, root, snapshot.clone())
        .await
        .expect("Put failed");

    // Retrieve
    let retrieved = runtime
        .snapshot_store("counters")
        .unwrap()
        .get("counters", DEFAULT_EDITION, root)
        .await
        .expect("Get failed");

    assert!(retrieved.is_some(), "Should retrieve snapshot");
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.sequence, 10, "Sequence should match");
    assert!(retrieved.state.is_some(), "State should exist");
}

#[tokio::test]
async fn test_snapshot_isolation_between_aggregates() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("counters", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();

    // Store snapshots for both
    let snapshot1 = Snapshot {
        sequence: 5,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"state-1".to_vec(),
        }),
    };

    let snapshot2 = Snapshot {
        sequence: 15,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"state-2".to_vec(),
        }),
    };

    runtime
        .snapshot_store("counters")
        .unwrap()
        .put("counters", DEFAULT_EDITION, root1, snapshot1)
        .await
        .expect("Put 1 failed");
    runtime
        .snapshot_store("counters")
        .unwrap()
        .put("counters", DEFAULT_EDITION, root2, snapshot2)
        .await
        .expect("Put 2 failed");

    // Verify isolation
    let ret1 = runtime
        .snapshot_store("counters")
        .unwrap()
        .get("counters", DEFAULT_EDITION, root1)
        .await
        .expect("Get 1 failed")
        .expect("Should exist");
    let ret2 = runtime
        .snapshot_store("counters")
        .unwrap()
        .get("counters", DEFAULT_EDITION, root2)
        .await
        .expect("Get 2 failed")
        .expect("Should exist");

    assert_eq!(ret1.sequence, 5, "Root1 sequence");
    assert_eq!(ret2.sequence, 15, "Root2 sequence");
}

#[tokio::test]
async fn test_snapshot_isolation_between_domains() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("domain_a", EchoAggregate::new())
        .register_aggregate("domain_b", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let root = Uuid::new_v4();

    // Store same root in different domains
    let snapshot_a = Snapshot {
        sequence: 100,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"domain-a".to_vec(),
        }),
    };

    let snapshot_b = Snapshot {
        sequence: 200,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"domain-b".to_vec(),
        }),
    };

    runtime
        .snapshot_store("domain_a")
        .unwrap()
        .put("domain_a", DEFAULT_EDITION, root, snapshot_a)
        .await
        .expect("Put A failed");
    runtime
        .snapshot_store("domain_b")
        .unwrap()
        .put("domain_b", DEFAULT_EDITION, root, snapshot_b)
        .await
        .expect("Put B failed");

    // Verify domain isolation
    let ret_a = runtime
        .snapshot_store("domain_a")
        .unwrap()
        .get("domain_a", DEFAULT_EDITION, root)
        .await
        .expect("Get A failed")
        .expect("Should exist");
    let ret_b = runtime
        .snapshot_store("domain_b")
        .unwrap()
        .get("domain_b", DEFAULT_EDITION, root)
        .await
        .expect("Get B failed")
        .expect("Should exist");

    assert_eq!(ret_a.sequence, 100, "Domain A sequence");
    assert_eq!(ret_b.sequence, 200, "Domain B sequence");
}

#[tokio::test]
async fn test_snapshot_update_overwrites() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("counters", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let root = Uuid::new_v4();

    // Store initial
    let snapshot1 = Snapshot {
        sequence: 5,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"initial".to_vec(),
        }),
    };

    runtime
        .snapshot_store("counters")
        .unwrap()
        .put("counters", DEFAULT_EDITION, root, snapshot1)
        .await
        .expect("Put 1 failed");

    // Update
    let snapshot2 = Snapshot {
        sequence: 10,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"updated".to_vec(),
        }),
    };

    runtime
        .snapshot_store("counters")
        .unwrap()
        .put("counters", DEFAULT_EDITION, root, snapshot2)
        .await
        .expect("Put 2 failed");

    // Verify updated
    let retrieved = runtime
        .snapshot_store("counters")
        .unwrap()
        .get("counters", DEFAULT_EDITION, root)
        .await
        .expect("Get failed")
        .expect("Should exist");

    assert_eq!(retrieved.sequence, 10, "Should be updated sequence");
    assert_eq!(
        retrieved.state.unwrap().value,
        b"updated".to_vec(),
        "Should be updated state"
    );
}

#[tokio::test]
async fn test_snapshot_delete() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("counters", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let root = Uuid::new_v4();

    // Store
    let snapshot = Snapshot {
        sequence: 5,
        state: Some(Any {
            type_url: "test.State".to_string(),
            value: b"to-delete".to_vec(),
        }),
    };

    runtime
        .snapshot_store("counters")
        .unwrap()
        .put("counters", DEFAULT_EDITION, root, snapshot)
        .await
        .expect("Put failed");

    // Verify exists
    assert!(
        runtime
            .snapshot_store("counters")
            .unwrap()
            .get("counters", DEFAULT_EDITION, root)
            .await
            .expect("Get failed")
            .is_some(),
        "Should exist before delete"
    );

    // Delete
    runtime
        .snapshot_store("counters")
        .unwrap()
        .delete("counters", DEFAULT_EDITION, root)
        .await
        .expect("Delete failed");

    // Verify gone
    assert!(
        runtime
            .snapshot_store("counters")
            .unwrap()
            .get("counters", DEFAULT_EDITION, root)
            .await
            .expect("Get failed")
            .is_none(),
        "Should not exist after delete"
    );
}
