use super::*;
use tempfile::TempDir;

fn test_config(dir: &TempDir, name: &str) -> CheckpointConfig {
    CheckpointConfig {
        file_path: dir.path().join(format!("checkpoint-{}.json", name)),
        flush_interval: Duration::from_millis(100),
        enabled: true,
    }
}

#[tokio::test]
async fn test_checkpoint_get_set() {
    let dir = TempDir::new().unwrap();
    let checkpoint = Checkpoint::new(test_config(&dir, "test"));

    let root = uuid::Uuid::new_v4().as_bytes().to_vec();

    // Initially none
    assert_eq!(checkpoint.get("orders", &root).await, None);

    // Update
    checkpoint.update("orders", &root, 5).await;
    assert_eq!(checkpoint.get("orders", &root).await, Some(5));

    // Update to higher value
    checkpoint.update("orders", &root, 10).await;
    assert_eq!(checkpoint.get("orders", &root).await, Some(10));

    // Lower value ignored
    checkpoint.update("orders", &root, 7).await;
    assert_eq!(checkpoint.get("orders", &root).await, Some(10));
}

#[tokio::test]
async fn test_checkpoint_should_process() {
    let dir = TempDir::new().unwrap();
    let checkpoint = Checkpoint::new(test_config(&dir, "test"));

    let root = uuid::Uuid::new_v4().as_bytes().to_vec();

    // All events should process initially
    assert!(checkpoint.should_process("orders", &root, 1).await);
    assert!(checkpoint.should_process("orders", &root, 5).await);

    // Mark sequence 5 as processed
    checkpoint.update("orders", &root, 5).await;

    // Events <= 5 should not process
    assert!(!checkpoint.should_process("orders", &root, 1).await);
    assert!(!checkpoint.should_process("orders", &root, 5).await);

    // Events > 5 should process
    assert!(checkpoint.should_process("orders", &root, 6).await);
    assert!(checkpoint.should_process("orders", &root, 10).await);
}

#[tokio::test]
async fn test_checkpoint_persistence() {
    let dir = TempDir::new().unwrap();
    let config = test_config(&dir, "persist");

    let root = uuid::Uuid::new_v4().as_bytes().to_vec();

    // Create and populate checkpoint
    {
        let checkpoint = Checkpoint::new(config.clone());
        checkpoint.update("orders", &root, 10).await;
        checkpoint.update("products", &root, 20).await;
        checkpoint.flush().await.unwrap();
    }

    // Load in new instance
    {
        let checkpoint = Checkpoint::new(config);
        checkpoint.load().await.unwrap();
        assert_eq!(checkpoint.get("orders", &root).await, Some(10));
        assert_eq!(checkpoint.get("products", &root).await, Some(20));
    }
}

#[tokio::test]
async fn test_checkpoint_disabled() {
    let checkpoint = Checkpoint::new(CheckpointConfig::disabled());

    let root = uuid::Uuid::new_v4().as_bytes().to_vec();

    // All operations are no-ops when disabled
    assert_eq!(checkpoint.get("orders", &root).await, None);
    checkpoint.update("orders", &root, 10).await;
    assert_eq!(checkpoint.get("orders", &root).await, None);
    assert!(checkpoint.should_process("orders", &root, 1).await);
}

#[tokio::test]
async fn test_checkpoint_stats() {
    let dir = TempDir::new().unwrap();
    let checkpoint = Checkpoint::new(test_config(&dir, "stats"));

    let root1 = uuid::Uuid::new_v4().as_bytes().to_vec();
    let root2 = uuid::Uuid::new_v4().as_bytes().to_vec();

    checkpoint.update("orders", &root1, 5).await;
    checkpoint.update("products", &root2, 10).await;

    let stats = checkpoint.stats().await;
    assert_eq!(stats.position_count, 2);
    assert!(stats.dirty);
}
