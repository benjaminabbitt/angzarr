//! Channel-based command bus implementation.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};

use super::config::CHANNEL_CAPACITY;
use crate::bus::error::Result;
use crate::bus::traits::{CommandBus, CommandHandler};
use crate::proto::CommandBook;

/// In-memory command bus using tokio broadcast channels.
///
/// Commands are published to a broadcast channel and routed to handlers
/// based on their domain. Used for async command execution in standalone mode.
pub struct ChannelCommandBus {
    /// Broadcast sender for publishing commands.
    sender: broadcast::Sender<Arc<CommandBook>>,
    /// Handlers by domain.
    handlers: Arc<RwLock<HashMap<String, Box<dyn CommandHandler>>>>,
    /// Flag indicating if consumer task is running.
    consuming: Arc<RwLock<bool>>,
}

impl ChannelCommandBus {
    /// Create a new channel command bus.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);

        info!("Channel command bus initialized");

        Self {
            sender,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            consuming: Arc::new(RwLock::new(false)),
        }
    }

    /// Start consuming commands (call after subscribe).
    pub async fn start_consuming(&self) -> Result<()> {
        // Check if already consuming
        {
            let mut consuming = self.consuming.write().await;
            if *consuming {
                return Ok(());
            }
            *consuming = true;
        }

        let mut receiver = self.sender.subscribe();
        let handlers = self.handlers.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(command) => {
                        let domain = command
                            .cover
                            .as_ref()
                            .map(|c| c.domain.clone())
                            .unwrap_or_else(|| "unknown".to_string());

                        debug!(
                            domain = %domain,
                            "Received command via channel"
                        );

                        // Find handler for domain
                        let handlers = handlers.read().await;
                        if let Some(handler) = handlers.get(&domain) {
                            if let Err(e) = handler.handle(command).await {
                                error!(
                                    domain = %domain,
                                    error = %e,
                                    "Command handler failed"
                                );
                            }
                        } else {
                            error!(
                                domain = %domain,
                                "No command handler registered for domain"
                            );
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        error!(
                            skipped = n,
                            "Command channel consumer lagged, skipped messages"
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Command channel closed, stopping consumer");
                        break;
                    }
                }
            }
        });

        info!("Channel command consumer started");

        Ok(())
    }
}

impl Default for ChannelCommandBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandBus for ChannelCommandBus {
    #[tracing::instrument(name = "command_bus.publish", skip_all, fields(domain = %command.cover.as_ref().map(|c| c.domain.as_str()).unwrap_or("unknown")))]
    async fn publish(&self, command: Arc<CommandBook>) -> Result<()> {
        let domain = command
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Send to channel (ignore error if no receivers)
        match self.sender.send(command) {
            Ok(receiver_count) => {
                debug!(
                    domain = %domain,
                    receivers = receiver_count,
                    "Published command to channel"
                );
            }
            Err(_) => {
                // No receivers - this is an error for commands
                error!(domain = %domain, "Failed to publish command (no receivers)");
            }
        }

        Ok(())
    }

    async fn subscribe(&self, domain: &str, handler: Box<dyn CommandHandler>) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(domain.to_string(), handler);

        info!(domain = %domain, "Command handler subscribed to channel bus");

        Ok(())
    }
}
