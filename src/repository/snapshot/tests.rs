use super::*;
use crate::proto::SnapshotRetention;
use crate::storage::mock::MockSnapshotStore;
use prost_types::Any;

fn test_snapshot(sequence: u32) -> Snapshot {
    Snapshot {
        sequence,
        state: Some(Any {
            type_url: "type.googleapis.com/TestState".to_string(),
            value: vec![10, 20, 30, sequence as u8],
        }),
        retention: SnapshotRetention::RetentionDefault as i32,
    }
}

#[tokio::test]
async fn test_get_returns_none_for_nonexistent() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let result = repo.get("orders", "test", Uuid::new_v4()).await.unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_put_and_get_roundtrip() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let root = Uuid::new_v4();
    let snapshot = test_snapshot(5);

    repo.put("orders", "test", root, snapshot.clone())
        .await
        .unwrap();

    let retrieved = repo.get("orders", "test", root).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().sequence, 5);
}

#[tokio::test]
async fn test_put_replaces_existing() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let root = Uuid::new_v4();

    repo.put("orders", "test", root, test_snapshot(3))
        .await
        .unwrap();
    repo.put("orders", "test", root, test_snapshot(7))
        .await
        .unwrap();

    let retrieved = repo.get("orders", "test", root).await.unwrap();
    assert_eq!(retrieved.unwrap().sequence, 7);
}

#[tokio::test]
async fn test_delete_removes_snapshot() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let root = Uuid::new_v4();

    repo.put("orders", "test", root, test_snapshot(5))
        .await
        .unwrap();
    repo.delete("orders", "test", root).await.unwrap();

    let retrieved = repo.get("orders", "test", root).await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_delete_nonexistent_succeeds() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let result = repo.delete("orders", "test", Uuid::new_v4()).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_domain_isolation() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let root = Uuid::new_v4();

    repo.put("orders", "test", root, test_snapshot(5))
        .await
        .unwrap();

    let other_domain = repo.get("customers", "test", root).await.unwrap();
    assert!(other_domain.is_none());
}

#[tokio::test]
async fn test_root_isolation() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::new(store);

    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();

    repo.put("orders", "test", root1, test_snapshot(5))
        .await
        .unwrap();

    let other_root = repo.get("orders", "test", root2).await.unwrap();
    assert!(other_root.is_none());
}

#[tokio::test]
async fn test_with_config_write_disabled_skips_put() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::with_config(store, false);

    let root = Uuid::new_v4();

    // Put should be a no-op
    repo.put("orders", "test", root, test_snapshot(5))
        .await
        .unwrap();

    // Get should return None since put was skipped
    let retrieved = repo.get("orders", "test", root).await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_with_config_write_enabled_writes() {
    let store = Arc::new(MockSnapshotStore::new());
    let repo = SnapshotRepository::with_config(store, true);

    let root = Uuid::new_v4();

    repo.put("orders", "test", root, test_snapshot(5))
        .await
        .unwrap();

    let retrieved = repo.get("orders", "test", root).await.unwrap();
    assert!(retrieved.is_some());
}
