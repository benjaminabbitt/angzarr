//! IPC event bus client - same interface as AMQP/Kafka.
//!
//! Publishers: Read subscriber list from env var, write directly to pipes.
//! Subscribers: Read from their named pipe.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nix::libc;

use async_trait::async_trait;
use prost::Message;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::broker::SubscriberInfo;
use super::checkpoint::{Checkpoint, CheckpointConfig};
use super::{DEFAULT_BASE_PATH, SUBSCRIBER_PIPE_PREFIX};
use crate::bus::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;
use crate::proto_ext::{CoverExt, EventPageExt};

// ============================================================================
// Consumer Helper Functions
// ============================================================================

/// Result of reading a message from a pipe.
#[derive(Debug)]
enum ReadResult {
    /// Message data read successfully.
    Message(Vec<u8>),
    /// Pipe closed (EOF) - should reopen.
    Eof,
    /// Message too large - should skip to next message.
    TooLarge(usize),
    /// Fatal error - should exit.
    Error(std::io::Error),
}

/// Read a length-prefixed message from a file.
///
/// Protocol: 4-byte big-endian length, then message body.
fn read_length_prefixed_message(file: &mut File) -> ReadResult {
    // Read length prefix (4 bytes, big-endian)
    let mut len_buf = [0u8; 4];
    match file.read_exact(&mut len_buf) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return ReadResult::Eof;
        }
        Err(e) => {
            return ReadResult::Error(e);
        }
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;
    if len > MAX_MESSAGE_SIZE {
        return ReadResult::TooLarge(len);
    }

    // Read message body
    let mut buf = vec![0u8; len];
    match file.read_exact(&mut buf) {
        Ok(_) => ReadResult::Message(buf),
        Err(e) => ReadResult::Error(e),
    }
}

/// Process a decoded EventBook with domain filtering, checkpoint, and handler dispatch.
///
/// Returns true if handlers should be called, false if message was filtered/skipped.
fn should_process_message(
    book: &EventBook,
    domains: &[String],
    checkpoint: &Checkpoint,
    rt: &tokio::runtime::Handle,
) -> bool {
    let routing_key = book.routing_key();

    // Check domain filter using routing key
    if !matches_domain_filter(&routing_key, domains) {
        return false;
    }

    // Extract root and max sequence for checkpoint
    let root_bytes = book
        .cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .map(|r| r.value.as_slice());
    let max_sequence = max_page_sequence(book);

    // Skip if already processed (checkpoint deduplication)
    if let (Some(root), Some(seq)) = (root_bytes, max_sequence) {
        let dominated =
            rt.block_on(async { !checkpoint.should_process(&routing_key, root, seq).await });
        if dominated {
            debug!(routing_key = %routing_key, sequence = seq, "Skipping checkpointed event");
            return false;
        }
    }

    true
}

/// Dispatch an EventBook to handlers and update checkpoint.
fn dispatch_to_handlers(
    book: Arc<EventBook>,
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    checkpoint: &Checkpoint,
    rt: &tokio::runtime::Handle,
) {
    let routing_key = book.routing_key();
    let root_bytes = book
        .cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .map(|r| r.value.clone());
    let max_sequence = max_page_sequence(&book);

    rt.block_on(async {
        let handlers_guard = handlers.read().await;
        for handler in handlers_guard.iter() {
            if let Err(e) = handler.handle(book.clone()).await {
                error!(error = %e, "Handler failed");
            }
        }

        // Update checkpoint after successful handler dispatch
        if let (Some(root), Some(seq)) = (&root_bytes, max_sequence) {
            checkpoint.update(&routing_key, root, seq).await;
        }
    });
}

/// Action to take after processing a message.
#[derive(Debug, PartialEq)]
enum MessageAction {
    /// Continue reading from the pipe.
    Continue,
    /// Break inner loop, reopen pipe (EOF or recoverable error).
    Reopen,
    /// Exit consumer entirely (fatal error).
    Exit,
}

/// Handle a successfully read message buffer.
///
/// Decodes, filters, and dispatches the message to handlers.
fn handle_message_buffer(
    buf: Vec<u8>,
    domains: &[String],
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    checkpoint: &Checkpoint,
    rt: &tokio::runtime::Handle,
) {
    let book = match EventBook::decode(&buf[..]) {
        Ok(b) => Arc::new(b),
        Err(e) => {
            error!(error = %e, "Failed to decode EventBook");
            return;
        }
    };

    if !should_process_message(&book, domains, checkpoint, rt) {
        return;
    }

    debug!(routing_key = %book.routing_key(), "Received event via pipe");
    dispatch_to_handlers(book, handlers, checkpoint, rt);
}

/// Flush checkpoint on pipe EOF.
fn flush_checkpoint_on_eof(checkpoint: &Checkpoint, rt: &tokio::runtime::Handle) {
    rt.block_on(async {
        if let Err(e) = checkpoint.flush().await {
            warn!(error = %e, "Failed to flush checkpoint on pipe EOF");
        }
    });
}

/// Process a single read result from the pipe.
///
/// Handles message decoding, filtering, and dispatching to handlers.
/// Returns an action indicating what the consumer loop should do next.
fn process_read_result(
    result: ReadResult,
    pipe_path: &std::path::Path,
    domains: &[String],
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    checkpoint: &Checkpoint,
    rt: &tokio::runtime::Handle,
) -> MessageAction {
    match result {
        ReadResult::Message(buf) => {
            handle_message_buffer(buf, domains, handlers, checkpoint, rt);
            MessageAction::Continue
        }
        ReadResult::Eof => {
            flush_checkpoint_on_eof(checkpoint, rt);
            debug!(pipe = %pipe_path.display(), "Pipe EOF, reopening");
            MessageAction::Reopen
        }
        ReadResult::TooLarge(len) => {
            error!(len, "Message too large");
            MessageAction::Reopen
        }
        ReadResult::Error(e) => {
            error!(error = %e, "Pipe read error");
            MessageAction::Exit
        }
    }
}

/// Read messages from a pipe connection until EOF or error.
///
/// Returns true to continue outer loop (reopen pipe), false to exit entirely.
fn handle_pipe_connection(
    file: &mut File,
    pipe_path: &std::path::Path,
    domains: &[String],
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    checkpoint: &Checkpoint,
    rt: &tokio::runtime::Handle,
) -> bool {
    loop {
        let result = read_length_prefixed_message(file);
        match process_read_result(result, pipe_path, domains, handlers, checkpoint, rt) {
            MessageAction::Continue => continue,
            MessageAction::Reopen => return true,
            MessageAction::Exit => return false,
        }
    }
}

/// Run the IPC consumer loop with reconnection logic.
///
/// This is the main consumer loop that:
/// 1. Opens the pipe (blocks until a writer connects)
/// 2. Reads messages until EOF
/// 3. Reopens the pipe and repeats (unless shutdown or fatal error)
fn run_consumer_loop(
    pipe_path: &std::path::Path,
    domains: &[String],
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    checkpoint: &Checkpoint,
    shutdown: &AtomicBool,
) {
    let rt = tokio::runtime::Handle::current();

    loop {
        if shutdown.load(Ordering::Relaxed) {
            debug!(pipe = %pipe_path.display(), "IPC consumer shutting down");
            return;
        }

        let mut file = match File::open(pipe_path) {
            Ok(f) => f,
            Err(e) => {
                error!(pipe = %pipe_path.display(), error = %e, "Failed to open pipe");
                return;
            }
        };

        // Check shutdown after unblocking from open
        if shutdown.load(Ordering::Relaxed) {
            debug!(pipe = %pipe_path.display(), "IPC consumer shutting down");
            return;
        }

        info!(pipe = %pipe_path.display(), "IPC consumer connected");

        if !handle_pipe_connection(&mut file, pipe_path, domains, handlers, checkpoint, &rt) {
            return; // Fatal error, exit entirely
        }
        // Otherwise continue loop to reopen pipe
    }
}

// ============================================================================
// Configuration
// ============================================================================

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
    /// Enable checkpoint persistence for subscribers.
    /// Tracks last-processed sequence per (domain, root) for crash recovery.
    pub checkpoint_enabled: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from(DEFAULT_BASE_PATH),
            subscriber_name: None,
            domains: Vec::new(),
            subscribers: Vec::new(),
            checkpoint_enabled: false,
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
            checkpoint_enabled: false,
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
            checkpoint_enabled: false,
        }
    }

    /// Create subscriber config with checkpointing enabled.
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
            checkpoint_enabled: true,
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
    /// Tracks last-processed sequence for crash recovery.
    checkpoint: Arc<Checkpoint>,
    /// Shutdown signal for the consumer task.
    shutdown: Arc<AtomicBool>,
}

impl IpcEventBus {
    /// Create a new IPC event bus.
    pub fn new(config: IpcConfig) -> Self {
        let checkpoint_config = match (&config.subscriber_name, config.checkpoint_enabled) {
            (Some(name), true) => CheckpointConfig::for_subscriber(&config.base_path, name),
            _ => CheckpointConfig::disabled(),
        };
        Self {
            checkpoint: Arc::new(Checkpoint::new(checkpoint_config)),
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            consumer_task: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
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

    /// Stop the consumer and clean up.
    ///
    /// Sets the shutdown flag and unblocks the consumer if it's stuck
    /// waiting for a writer on the pipe.
    pub async fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);

        // Open the pipe for writing to unblock consumer's blocking File::open().
        // The consumer will see the shutdown flag after the open returns.
        if let Some(pipe_path) = self.config.subscriber_pipe() {
            let _ = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&pipe_path);
        }

        if let Some(handle) = self.consumer_task.write().await.take() {
            handle.abort();
        }
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

        // Load persisted checkpoint positions before starting consumer
        if let Err(e) = self.checkpoint.load().await {
            warn!(error = %e, "Failed to load checkpoint, starting fresh");
        }

        let handlers = self.handlers.clone();
        let domains = self.config.domains.clone();
        let checkpoint = self.checkpoint.clone();
        let shutdown = self.shutdown.clone();

        info!(pipe = %pipe_path.display(), "Starting IPC consumer");

        // Spawn blocking task for pipe reading (pipes are blocking I/O)
        let handle = tokio::task::spawn_blocking(move || {
            run_consumer_loop(&pipe_path, &domains, &handlers, &checkpoint, &shutdown);
        });

        *self.consumer_task.write().await = Some(handle);

        Ok(())
    }
}

#[async_trait]
impl EventBus for IpcEventBus {
    /// Publish events directly to subscriber pipes.
    #[tracing::instrument(name = "bus.publish", skip_all, fields(domain = %book.domain()))]
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        if self.config.subscribers.is_empty() {
            debug!("No subscribers configured, event not published");
            return Ok(PublishResult::default());
        }

        let routing_key = book.routing_key();

        // Serialize once
        let serialized = book.encode_to_vec();
        let len_bytes = (serialized.len() as u32).to_be_bytes();

        for subscriber in &self.config.subscribers {
            // Check domain filter using routing key
            if !matches_domain_filter(&routing_key, &subscriber.domains) {
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
                            routing_key = %routing_key,
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

    /// Create a new subscriber bus sharing the same base path.
    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let domains = match domain_filter {
            Some(d) => vec![d.to_string()],
            None => vec![],
        };
        let config = IpcConfig::subscriber(&self.config.base_path, name, domains);
        Ok(Arc::new(IpcEventBus::new(config)))
    }
}

/// Extract the highest sequence number from an EventBook's pages.
fn max_page_sequence(book: &EventBook) -> Option<u32> {
    book.pages.iter().map(|p| p.sequence_num()).max()
}

/// Domain filter helper for testing - checks if routing_key matches domain list.
///
/// Returns true if:
/// - domains is empty (accept all)
/// - domains contains "#" (wildcard)
/// - domains contains the routing_key
fn matches_domain_filter(routing_key: &str, domains: &[String]) -> bool {
    domains.is_empty() || domains.iter().any(|d| d == "#" || d == routing_key)
}

#[cfg(test)]
#[path = "client.test.rs"]
mod tests;
