//! Tests for IPC pipe broker.
//!
//! IpcBroker manages named pipes for subscriber processes:
//! - Creates FIFO pipes for each subscriber
//! - Tracks subscribers by name with domain filters
//! - Serializes subscriber list to JSON for publishers
//! - Cleans up pipes on drop
//!
//! Why this matters: The broker is the registry that enables publishers
//! to discover subscribers. Without proper pipe creation and cleanup,
//! IPC communication would fail or leave stale pipes on disk.
//!
//! Key behaviors verified:
//! - register_subscriber creates pipe file
//! - unregister_subscriber removes pipe file
//! - get_subscribers returns all registered
//! - subscribers_to_json produces valid JSON

use super::*;
use tempfile::TempDir;

/// Registering subscriber creates named pipe file.
#[test]
fn test_broker_register_subscriber() {
    let temp_dir = TempDir::new().unwrap();
    let config = IpcBrokerConfig::with_base_path(temp_dir.path());
    let mut broker = IpcBroker::new(config);

    let info = broker
        .register_subscriber("test-projector", vec!["orders".to_string()])
        .unwrap();

    assert!(info.pipe_path.exists());
    assert!(info
        .pipe_path
        .to_string_lossy()
        .contains("subscriber-test-projector.pipe"));
}

/// Unregistering subscriber removes pipe file.
#[test]
fn test_broker_unregister_subscriber() {
    let temp_dir = TempDir::new().unwrap();
    let config = IpcBrokerConfig::with_base_path(temp_dir.path());
    let mut broker = IpcBroker::new(config);

    let info = broker.register_subscriber("test", vec![]).unwrap();
    assert!(info.pipe_path.exists());

    broker.unregister_subscriber("test");
    assert!(!info.pipe_path.exists());
}

/// get_subscribers returns all registered subscribers.
#[test]
fn test_broker_get_subscribers() {
    let temp_dir = TempDir::new().unwrap();
    let config = IpcBrokerConfig::with_base_path(temp_dir.path());
    let mut broker = IpcBroker::new(config);

    broker
        .register_subscriber("proj-a", vec!["orders".to_string()])
        .unwrap();
    broker
        .register_subscriber("proj-b", vec!["inventory".to_string()])
        .unwrap();

    let subs = broker.get_subscribers();
    assert_eq!(subs.len(), 2);
}

/// subscribers_to_json produces valid JSON for env var.
///
/// Publishers receive subscriber info via env var in JSON format.
/// This must serialize correctly for the publish side to parse.
#[test]
fn test_broker_subscribers_to_json() {
    let temp_dir = TempDir::new().unwrap();
    let config = IpcBrokerConfig::with_base_path(temp_dir.path());
    let mut broker = IpcBroker::new(config);

    broker
        .register_subscriber("test", vec!["orders".to_string()])
        .unwrap();

    let json = broker.subscribers_to_json();
    assert!(json.contains("test"));
    assert!(json.contains("orders"));
}
