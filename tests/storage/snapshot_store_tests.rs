//! SnapshotStore interface tests.
//!
//! These tests verify the contract of the SnapshotStore trait.
//! Each storage implementation should run these tests.

use prost_types::Any;
use uuid::Uuid;

use angzarr::proto::Snapshot;
use angzarr::storage::SnapshotStore;

/// Create a test snapshot at the given sequence.
pub fn make_snapshot(seq: u32) -> Snapshot {
    Snapshot {
        sequence: seq,
        state: Some(Any {
            type_url: format!("type.example/TestState{}", seq),
            value: vec![10, 20, 30, seq as u8],
        }),
    }
}

/// Create a snapshot with custom data for verification.
pub fn make_snapshot_with_data(seq: u32, data: Vec<u8>) -> Snapshot {
    Snapshot {
        sequence: seq,
        state: Some(Any {
            type_url: "type.example/CustomState".to_string(),
            value: data,
        }),
    }
}

// =============================================================================
// SnapshotStore::get tests
// =============================================================================

pub async fn test_get_nonexistent<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_nonexist";
    let root = Uuid::new_v4();

    let snapshot = store
        .get(domain, root)
        .await
        .expect("get should succeed");
    assert!(snapshot.is_none(), "nonexistent snapshot should be None");
}

pub async fn test_get_existing<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_exist";
    let root = Uuid::new_v4();

    store
        .put(domain, root, make_snapshot(10))
        .await
        .expect("put should succeed");

    let snapshot = store
        .get(domain, root)
        .await
        .expect("get should succeed")
        .expect("snapshot should exist");

    assert_eq!(snapshot.sequence, 10);
}

pub async fn test_get_preserves_data<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_data";
    let root = Uuid::new_v4();
    let data = vec![1, 2, 3, 4, 5, 100, 200, 255];

    store
        .put(domain, root, make_snapshot_with_data(5, data.clone()))
        .await
        .expect("put should succeed");

    let snapshot = store
        .get(domain, root)
        .await
        .expect("get should succeed")
        .expect("snapshot should exist");

    assert_eq!(snapshot.sequence, 5);
    let state = snapshot.state.expect("state should exist");
    assert_eq!(state.type_url, "type.example/CustomState");
    assert_eq!(state.value, data);
}

// =============================================================================
// SnapshotStore::put tests
// =============================================================================

pub async fn test_put_new<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_put_new";
    let root = Uuid::new_v4();

    store
        .put(domain, root, make_snapshot(5))
        .await
        .expect("put should succeed");

    let snapshot = store
        .get(domain, root)
        .await
        .expect("get should succeed")
        .expect("snapshot should exist");

    assert_eq!(snapshot.sequence, 5);
}

pub async fn test_put_update<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_put_upd";
    let root = Uuid::new_v4();

    // Initial snapshot
    store
        .put(domain, root, make_snapshot(5))
        .await
        .expect("first put should succeed");

    // Update snapshot
    store
        .put(domain, root, make_snapshot(15))
        .await
        .expect("second put should succeed");

    let snapshot = store
        .get(domain, root)
        .await
        .expect("get should succeed")
        .expect("snapshot should exist");

    assert_eq!(snapshot.sequence, 15, "should have updated sequence");
}

pub async fn test_put_multiple_updates<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_multi_upd";
    let root = Uuid::new_v4();

    for seq in [1, 5, 10, 20, 50] {
        store
            .put(domain, root, make_snapshot(seq))
            .await
            .expect("put should succeed");

        let snapshot = store
            .get(domain, root)
            .await
            .expect("get should succeed")
            .expect("snapshot should exist");

        assert_eq!(snapshot.sequence, seq, "sequence should be {}", seq);
    }
}

// =============================================================================
// SnapshotStore::delete tests
// =============================================================================

pub async fn test_delete_existing<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_del_exist";
    let root = Uuid::new_v4();

    store
        .put(domain, root, make_snapshot(10))
        .await
        .expect("put should succeed");

    // Verify it exists
    assert!(store.get(domain, root).await.unwrap().is_some());

    store
        .delete(domain, root)
        .await
        .expect("delete should succeed");

    // Verify it's gone
    let snapshot = store.get(domain, root).await.expect("get should succeed");
    assert!(snapshot.is_none(), "deleted snapshot should be None");
}

pub async fn test_delete_nonexistent<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_del_none";
    let root = Uuid::new_v4();

    // Delete non-existent should succeed (idempotent)
    store
        .delete(domain, root)
        .await
        .expect("delete nonexistent should succeed");
}

pub async fn test_delete_then_recreate<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_recreate";
    let root = Uuid::new_v4();

    store.put(domain, root, make_snapshot(5)).await.unwrap();
    store.delete(domain, root).await.unwrap();
    store.put(domain, root, make_snapshot(20)).await.unwrap();

    let snapshot = store
        .get(domain, root)
        .await
        .expect("get should succeed")
        .expect("recreated snapshot should exist");

    assert_eq!(snapshot.sequence, 20);
}

// =============================================================================
// Isolation tests
// =============================================================================

pub async fn test_aggregate_isolation<S: SnapshotStore>(store: &S) {
    let domain = "test_snap_iso";
    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();

    store.put(domain, root1, make_snapshot(10)).await.unwrap();
    store.put(domain, root2, make_snapshot(20)).await.unwrap();

    let snap1 = store.get(domain, root1).await.unwrap().unwrap();
    let snap2 = store.get(domain, root2).await.unwrap().unwrap();

    assert_eq!(snap1.sequence, 10);
    assert_eq!(snap2.sequence, 20);

    // Delete one doesn't affect the other
    store.delete(domain, root1).await.unwrap();

    assert!(store.get(domain, root1).await.unwrap().is_none());
    assert!(store.get(domain, root2).await.unwrap().is_some());
}

pub async fn test_domain_isolation<S: SnapshotStore>(store: &S) {
    let domain1 = "test_snap_d1";
    let domain2 = "test_snap_d2";
    let root = Uuid::new_v4();

    store.put(domain1, root, make_snapshot(10)).await.unwrap();
    store.put(domain2, root, make_snapshot(20)).await.unwrap();

    let snap1 = store.get(domain1, root).await.unwrap().unwrap();
    let snap2 = store.get(domain2, root).await.unwrap().unwrap();

    assert_eq!(snap1.sequence, 10);
    assert_eq!(snap2.sequence, 20);
}

// =============================================================================
// Test runner macro
// =============================================================================

/// Run all SnapshotStore interface tests against a store implementation.
#[macro_export]
macro_rules! run_snapshot_store_tests {
    ($store:expr) => {
        use $crate::storage::snapshot_store_tests::*;

        // get tests
        test_get_nonexistent($store).await;
        println!("  test_get_nonexistent: PASSED");

        test_get_existing($store).await;
        println!("  test_get_existing: PASSED");

        test_get_preserves_data($store).await;
        println!("  test_get_preserves_data: PASSED");

        // put tests
        test_put_new($store).await;
        println!("  test_put_new: PASSED");

        test_put_update($store).await;
        println!("  test_put_update: PASSED");

        test_put_multiple_updates($store).await;
        println!("  test_put_multiple_updates: PASSED");

        // delete tests
        test_delete_existing($store).await;
        println!("  test_delete_existing: PASSED");

        test_delete_nonexistent($store).await;
        println!("  test_delete_nonexistent: PASSED");

        test_delete_then_recreate($store).await;
        println!("  test_delete_then_recreate: PASSED");

        // isolation tests
        test_aggregate_isolation($store).await;
        println!("  test_aggregate_isolation: PASSED");

        test_domain_isolation($store).await;
        println!("  test_domain_isolation: PASSED");
    };
}
