//! IPC event bus client - same interface as AMQP/Kafka.
//!
//! Publishers: Read subscriber list from env var, write directly to pipes.
//! Subscribers: Read from their named pipe.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use nix::libc;

use async_trait::async_trait;
use prost::Message;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::broker::SubscriberInfo;
use super::{DEFAULT_BASE_PATH, SUBSCRIBER_PIPE_PREFIX};
use crate::bus::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;

/// Env var name for subscriber list (set by orchestrator).
pub const SUBSCRIBERS_ENV_VAR: &str = "ANGZARR_IPC_SUBSCRIBERS";

/// Configuration for IPC event bus.
#[derive(Debug, Clone)]
pub struct IpcConfig {
    /// Base path for pipes.
    pub base_path: PathBuf,
    /// Subscriber name (for subscriber mode only).
    pub subscriber_name: Option<String>,
    /// Domains to subscribe to (for subscriber mode only).
    pub domains: Vec<String>,
    /// Subscriber list (for publisher mode, loaded from env var).
    pub subscribers: Vec<SubscriberInfo>,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from(DEFAULT_BASE_PATH),
            subscriber_name: None,
            domains: Vec::new(),
            subscribers: Vec::new(),
        }
    }
}

impl IpcConfig {
    /// Create publisher config, loading subscribers from env var.
    pub fn publisher(base_path: impl Into<PathBuf>) -> Self {
        let subscribers = load_subscribers_from_env();
        Self {
            base_path: base_path.into(),
            subscriber_name: None,
            domains: Vec::new(),
            subscribers,
        }
    }

    /// Create publisher config with explicit subscriber list.
    pub fn publisher_with_subscribers(
        base_path: impl Into<PathBuf>,
        subscribers: Vec<SubscriberInfo>,
    ) -> Self {
        Self {
            base_path: base_path.into(),
            subscriber_name: None,
            domains: Vec::new(),
            subscribers,
        }
    }

    /// Create subscriber config.
    pub fn subscriber(
        base_path: impl Into<PathBuf>,
        name: impl Into<String>,
        domains: Vec<String>,
    ) -> Self {
        Self {
            base_path: base_path.into(),
            subscriber_name: Some(name.into()),
            domains,
            subscribers: Vec::new(),
        }
    }

    /// Get the subscriber pipe path.
    pub fn subscriber_pipe(&self) -> Option<PathBuf> {
        self.subscriber_name.as_ref().map(|name| {
            self.base_path
                .join(format!("{}{}.pipe", SUBSCRIBER_PIPE_PREFIX, name))
        })
    }
}

/// Load subscriber list from env var.
fn load_subscribers_from_env() -> Vec<SubscriberInfo> {
    match std::env::var(SUBSCRIBERS_ENV_VAR) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            warn!(error = %e, "Failed to parse {}", SUBSCRIBERS_ENV_VAR);
            Vec::new()
        }),
        Err(_) => {
            debug!("{} not set, no subscribers configured", SUBSCRIBERS_ENV_VAR);
            Vec::new()
        }
    }
}

/// IPC event bus - same interface as AMQP/Kafka.
pub struct IpcEventBus {
    config: IpcConfig,
    /// Handlers for subscriber mode.
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    /// Consumer task handle.
    consumer_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl IpcEventBus {
    /// Create a new IPC event bus.
    pub fn new(config: IpcConfig) -> Self {
        Self {
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            consumer_task: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a publisher bus (loads subscribers from env var).
    pub fn publisher(base_path: impl Into<PathBuf>) -> Self {
        Self::new(IpcConfig::publisher(base_path))
    }

    /// Create a subscriber bus.
    pub fn subscriber(
        base_path: impl Into<PathBuf>,
        name: impl Into<String>,
        domains: Vec<String>,
    ) -> Self {
        Self::new(IpcConfig::subscriber(base_path, name, domains))
    }

    /// Start consuming from the pipe (for subscribers).
    pub async fn start_consuming(&self) -> Result<()> {
        let pipe_path = match self.config.subscriber_pipe() {
            Some(p) => p,
            None => {
                return Err(BusError::Subscribe(
                    "No subscriber name configured".to_string(),
                ))
            }
        };

        // Check if already consuming
        {
            let task = self.consumer_task.read().await;
            if task.is_some() {
                return Ok(());
            }
        }

        let handlers = self.handlers.clone();
        let domains = self.config.domains.clone();

        info!(pipe = %pipe_path.display(), "Starting IPC consumer");

        // Spawn blocking task for pipe reading (pipes are blocking I/O)
        let handle = tokio::task::spawn_blocking(move || {
            // Outer loop: reopen pipe after EOF (writers may close and reopen)
            loop {
                // Open pipe for reading (blocks until writer opens)
                let mut file = match File::open(&pipe_path) {
                    Ok(f) => f,
                    Err(e) => {
                        error!(pipe = %pipe_path.display(), error = %e, "Failed to open pipe");
                        return;
                    }
                };

                info!(pipe = %pipe_path.display(), "IPC consumer connected");

                // Inner loop: read messages until EOF
                loop {
                    // Read length prefix (4 bytes, big-endian)
                    let mut len_buf = [0u8; 4];
                    match file.read_exact(&mut len_buf) {
                        Ok(_) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            // Pipe closed by writers, reopen and wait for more
                            debug!(pipe = %pipe_path.display(), "Pipe EOF, reopening");
                            break; // Break inner loop, continue outer to reopen
                        }
                        Err(e) => {
                            error!(error = %e, "Pipe read error");
                            return; // Fatal error, exit entirely
                        }
                    }

                let len = u32::from_be_bytes(len_buf) as usize;
                if len > 10 * 1024 * 1024 {
                    error!(len, "Message too large");
                    break;
                }

                // Read message body
                let mut buf = vec![0u8; len];
                if let Err(e) = file.read_exact(&mut buf) {
                    error!(error = %e, "Failed to read message body");
                    break;
                }

                // Decode EventBook
                let book = match EventBook::decode(&buf[..]) {
                    Ok(b) => Arc::new(b),
                    Err(e) => {
                        error!(error = %e, "Failed to decode EventBook");
                        continue;
                    }
                };

                let domain = book
                    .cover
                    .as_ref()
                    .map(|c| c.domain.as_str())
                    .unwrap_or("unknown");

                // Check domain filter
                let matches = domains.is_empty() || domains.iter().any(|d| d == "#" || d == domain);

                if !matches {
                    continue;
                }

                debug!(domain = %domain, "Received event via pipe");

                // Call handlers
                let handlers_clone = handlers.clone();
                let book_clone = book.clone();
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let handlers_guard = handlers_clone.read().await;
                    for handler in handlers_guard.iter() {
                        if let Err(e) = handler.handle(book_clone.clone()).await {
                            error!(error = %e, "Handler failed");
                        }
                    }
                });
                }
            }
        });

        *self.consumer_task.write().await = Some(handle);

        Ok(())
    }
}

#[async_trait]
impl EventBus for IpcEventBus {
    /// Publish events directly to subscriber pipes.
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        if self.config.subscribers.is_empty() {
            debug!("No subscribers configured, event not published");
            return Ok(PublishResult::default());
        }

        let domain = book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        // Serialize once
        let serialized = book.encode_to_vec();
        let len_bytes = (serialized.len() as u32).to_be_bytes();

        for subscriber in &self.config.subscribers {
            // Check domain filter
            let matches = subscriber.domains.is_empty()
                || subscriber.domains.iter().any(|d| d == "#" || d == domain);

            if !matches {
                continue;
            }

            // Open pipe and write (non-blocking to avoid deadlock)
            match OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&subscriber.pipe_path)
            {
                Ok(mut file) => {
                    if let Err(e) = file
                        .write_all(&len_bytes)
                        .and_then(|_| file.write_all(&serialized))
                    {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            warn!(subscriber = %subscriber.name, "Pipe full, dropping event");
                        } else if e.kind() != std::io::ErrorKind::BrokenPipe {
                            warn!(
                                subscriber = %subscriber.name,
                                error = %e,
                                "Failed to write to pipe"
                            );
                        }
                    } else {
                        debug!(
                            subscriber = %subscriber.name,
                            domain = %domain,
                            "Published event to pipe"
                        );
                    }
                }
                Err(e) => {
                    // ENXIO = no reader yet, that's okay
                    if e.raw_os_error() != Some(libc::ENXIO) {
                        warn!(
                            subscriber = %subscriber.name,
                            error = %e,
                            "Failed to open pipe"
                        );
                    }
                }
            }
        }

        Ok(PublishResult::default())
    }

    /// Subscribe to events from the named pipe.
    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        if self.config.subscriber_name.is_none() {
            return Err(BusError::Subscribe(
                "Cannot subscribe without subscriber_name".to_string(),
            ));
        }

        let count = {
            let mut handlers = self.handlers.write().await;
            handlers.push(handler);
            handlers.len()
        };

        info!(handler_count = count, "Handler subscribed to IPC bus");

        Ok(())
    }

    /// Start consuming from the pipe (IPC requires explicit start).
    async fn start_consuming(&self) -> Result<()> {
        IpcEventBus::start_consuming(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_config_publisher() {
        let config = IpcConfig::publisher("/tmp/test");
        assert_eq!(config.base_path, PathBuf::from("/tmp/test"));
        assert!(config.subscriber_name.is_none());
    }

    #[test]
    fn test_ipc_config_subscriber() {
        let config = IpcConfig::subscriber("/tmp/test", "my-projector", vec!["orders".to_string()]);
        assert_eq!(config.base_path, PathBuf::from("/tmp/test"));
        assert_eq!(config.subscriber_name, Some("my-projector".to_string()));
        assert_eq!(config.domains, vec!["orders".to_string()]);
        assert_eq!(
            config.subscriber_pipe(),
            Some(PathBuf::from("/tmp/test/subscriber-my-projector.pipe"))
        );
    }

    #[test]
    fn test_ipc_config_publisher_with_subscribers() {
        let subs = vec![SubscriberInfo {
            name: "test".to_string(),
            domains: vec!["orders".to_string()],
            pipe_path: PathBuf::from("/tmp/test.pipe"),
        }];
        let config = IpcConfig::publisher_with_subscribers("/tmp/test", subs);
        assert_eq!(config.subscribers.len(), 1);
    }
}
