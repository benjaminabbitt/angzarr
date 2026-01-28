//! IPC pipe registry for embedded mode.
//!
//! Creates and manages named pipes for subscriber processes.
//! No socket - publishers write directly to pipes.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::info;

use super::{DEFAULT_BASE_PATH, SUBSCRIBER_PIPE_PREFIX};

/// Configuration for the IPC broker.
#[derive(Debug, Clone)]
pub struct IpcBrokerConfig {
    /// Base path for pipes.
    pub base_path: PathBuf,
}

impl Default for IpcBrokerConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from(DEFAULT_BASE_PATH),
        }
    }
}

impl IpcBrokerConfig {
    /// Create config with custom base path.
    pub fn with_base_path(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }
}

/// Subscriber info passed to publishers via env var.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberInfo {
    /// Subscriber name.
    pub name: String,
    /// Domains this subscriber is interested in (empty = all).
    pub domains: Vec<String>,
    /// Path to the subscriber's named pipe.
    pub pipe_path: PathBuf,
}

/// IPC pipe registry - creates and manages subscriber pipes.
///
/// Used by the orchestrator to:
/// 1. Register subscribers and create their pipes
/// 2. Get subscriber list to pass to publishers via env var
/// 3. Clean up pipes on shutdown
pub struct IpcBroker {
    config: IpcBrokerConfig,
    /// Registered subscribers.
    subscribers: HashMap<String, SubscriberInfo>,
}

impl IpcBroker {
    /// Create a new IPC broker.
    pub fn new(config: IpcBrokerConfig) -> Self {
        // Create base directory
        let _ = fs::create_dir_all(&config.base_path);

        Self {
            config,
            subscribers: HashMap::new(),
        }
    }

    /// Get the base path for pipes.
    pub fn base_path(&self) -> &PathBuf {
        &self.config.base_path
    }

    /// Register a subscriber and create its named pipe.
    pub fn register_subscriber(
        &mut self,
        name: &str,
        domains: Vec<String>,
    ) -> std::io::Result<SubscriberInfo> {
        let pipe_path = self
            .config
            .base_path
            .join(format!("{}{}.pipe", SUBSCRIBER_PIPE_PREFIX, name));

        // Remove existing pipe if any
        if pipe_path.exists() {
            fs::remove_file(&pipe_path)?;
        }

        // Create named pipe (FIFO)
        #[cfg(unix)]
        {
            use nix::sys::stat::Mode;
            use nix::unistd::mkfifo;

            mkfifo(&pipe_path, Mode::S_IRUSR | Mode::S_IWUSR).map_err(std::io::Error::other)?;
        }

        info!(
            subscriber = %name,
            pipe = %pipe_path.display(),
            domains = ?domains,
            "Registered subscriber"
        );

        let info = SubscriberInfo {
            name: name.to_string(),
            domains,
            pipe_path,
        };

        self.subscribers.insert(name.to_string(), info.clone());

        Ok(info)
    }

    /// Unregister a subscriber and remove its pipe.
    pub fn unregister_subscriber(&mut self, name: &str) {
        if let Some(info) = self.subscribers.remove(name) {
            if info.pipe_path.exists() {
                let _ = fs::remove_file(&info.pipe_path);
            }
            info!(subscriber = %name, "Unregistered subscriber");
        }
    }

    /// Get all registered subscribers (for passing to publishers).
    pub fn get_subscribers(&self) -> Vec<SubscriberInfo> {
        self.subscribers.values().cloned().collect()
    }

    /// Serialize subscriber list to JSON for env var.
    pub fn subscribers_to_json(&self) -> String {
        serde_json::to_string(&self.get_subscribers()).unwrap_or_else(|_| "[]".to_string())
    }

    /// Cleanup all subscriber pipes.
    pub fn cleanup(&self) {
        for info in self.subscribers.values() {
            if info.pipe_path.exists() {
                let _ = fs::remove_file(&info.pipe_path);
            }
        }
        info!("Cleaned up {} subscriber pipes", self.subscribers.len());
    }
}

impl Drop for IpcBroker {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}
