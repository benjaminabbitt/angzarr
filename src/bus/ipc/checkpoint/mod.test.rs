//! Tests for IPC bus checkpoint/position tracking.
//!
//! Checkpoints track the last processed sequence per (domain, root) pair.
//! This enables at-least-once delivery with deduplication: if a subscriber
//! crashes and restarts, it can skip events already processed.
//!
//! Key behaviors:
//! - Higher sequences replace lower ones (high-water mark)
//! - should_process() returns false for already-processed events
//! - State persists to disk and survives restarts
//! - Disabled mode makes all operations no-ops
//!
//! Without checkpointing, restarts would reprocess all events from the
//! beginning, potentially causing duplicate side effects.

use super::*;
use tempfile::TempDir;

fn test_config(dir: &TempDir, name: &str) -> CheckpointConfig {
    CheckpointConfig {
        file_path: dir.path().join(format!("checkpoint-{}.json", name)),
        flush_interval: Duration::from_millis(100),
        enabled: true,
    }
}

// ============================================================================
// Core Checkpoint Operations
// ============================================================================

/// Checkpoint tracks high-water mark per (domain, root).
///
/// Lower sequence updates are ignored — we only advance, never regress.
/// This models "I've seen up to sequence N" semantics.
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

/// should_process enables deduplication during message handling.
///
/// Events with sequence <= checkpoint are skipped. This prevents duplicate
/// processing after subscriber restart when replaying from a pipe.
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

// ============================================================================
// Persistence Tests
// ============================================================================

/// Checkpoint state survives process restarts.
///
/// flush() writes to disk; load() restores on startup. Without persistence,
/// subscribers would lose their position on crash and reprocess everything.
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

// ============================================================================
// Configuration Tests
// ============================================================================

/// Disabled checkpoint makes all operations no-ops.
///
/// Used when deduplication isn't needed (e.g., idempotent handlers) or
/// when you want to force reprocessing of all events.
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

/// Stats report checkpoint health metrics.
///
/// position_count shows how many (domain, root) pairs are tracked.
/// dirty indicates unsaved changes pending flush.
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
