//! In-process event bus implementation.
//!
//! Routes events to in-process projectors and sagas without gRPC overhead.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::interfaces::event_bus::{BusError, EventBus, EventHandler, Result};
use crate::interfaces::projector::Projector;
use crate::interfaces::saga::Saga;
use crate::proto::{CommandBook, EventBook};

/// In-process event bus.
///
/// Routes events directly to registered projectors and sagas
/// without network overhead. Ideal for:
/// - Single-process applications
/// - Testing
/// - Embedded use cases
pub struct InProcessEventBus {
    projectors: RwLock<Vec<Arc<dyn Projector>>>,
    sagas: RwLock<Vec<Arc<dyn Saga>>>,
    /// Commands produced by sagas, to be processed by caller.
    pending_commands: RwLock<Vec<CommandBook>>,
}

impl InProcessEventBus {
    /// Create a new in-process event bus.
    pub fn new() -> Self {
        Self {
            projectors: RwLock::new(Vec::new()),
            sagas: RwLock::new(Vec::new()),
            pending_commands: RwLock::new(Vec::new()),
        }
    }

    /// Register an in-process projector.
    pub async fn add_projector(&self, projector: Box<dyn Projector>) {
        let projector: Arc<dyn Projector> = projector.into();
        info!(
            projector.name = %projector.name(),
            projector.domains = ?projector.domains(),
            "Registered in-process projector"
        );
        self.projectors.write().await.push(projector);
    }

    /// Register an in-process saga.
    pub async fn add_saga(&self, saga: Box<dyn Saga>) {
        let saga: Arc<dyn Saga> = saga.into();
        info!(
            saga.name = %saga.name(),
            saga.domains = ?saga.domains(),
            "Registered in-process saga"
        );
        self.sagas.write().await.push(saga);
    }

    /// Take any commands produced by sagas during publish.
    ///
    /// Call this after `publish()` to get commands that need processing.
    pub async fn take_pending_commands(&self) -> Vec<CommandBook> {
        std::mem::take(&mut *self.pending_commands.write().await)
    }

    /// Get the domain from an event book.
    fn get_domain(book: &EventBook) -> Option<&str> {
        book.cover.as_ref().map(|c| c.domain.as_str())
    }

    /// Check if a handler is interested in this domain.
    fn is_interested(handler_domains: &[String], event_domain: &str) -> bool {
        handler_domains.is_empty() || handler_domains.iter().any(|d| d == event_domain)
    }
}

impl Default for InProcessEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for InProcessEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<()> {
        let domain = Self::get_domain(&book).unwrap_or("unknown");

        // Collect projectors under read lock, then release before async calls
        let projectors: Vec<_> = {
            let guard = self.projectors.read().await;
            guard
                .iter()
                .filter(|p| Self::is_interested(&p.domains(), domain))
                .cloned()
                .collect()
        };

        for projector in projectors {
            match projector.project(&book).await {
                Ok(Some(_projection)) => {
                    info!(
                        projector.name = %projector.name(),
                        domain = %domain,
                        "Projection produced"
                    );
                }
                Ok(None) => {
                    info!(
                        projector.name = %projector.name(),
                        domain = %domain,
                        "Projection completed"
                    );
                }
                Err(e) => {
                    if projector.is_synchronous() {
                        error!(
                            projector.name = %projector.name(),
                            error = %e,
                            "Synchronous projector failed"
                        );
                        return Err(BusError::ProjectorFailed {
                            name: projector.name().to_string(),
                            source: e,
                        });
                    }
                    warn!(
                        projector.name = %projector.name(),
                        error = %e,
                        "Async projector failed"
                    );
                }
            }
        }

        // Collect sagas under read lock, then release before async calls
        let sagas: Vec<_> = {
            let guard = self.sagas.read().await;
            guard
                .iter()
                .filter(|s| Self::is_interested(&s.domains(), domain))
                .cloned()
                .collect()
        };

        // Collect all commands first, then add to pending in one write
        let mut all_commands = Vec::new();

        for saga in sagas {
            match saga.handle(&book).await {
                Ok(commands) => {
                    if !commands.is_empty() {
                        info!(
                            saga.name = %saga.name(),
                            command_count = commands.len(),
                            "Saga produced commands"
                        );
                        all_commands.extend(commands);
                    }
                }
                Err(e) => {
                    if saga.is_synchronous() {
                        error!(
                            saga.name = %saga.name(),
                            error = %e,
                            "Synchronous saga failed"
                        );
                        return Err(BusError::SagaFailed {
                            name: saga.name().to_string(),
                            source: e,
                        });
                    }
                    warn!(
                        saga.name = %saga.name(),
                        error = %e,
                        "Async saga failed"
                    );
                }
            }
        }

        // Single write to pending_commands
        if !all_commands.is_empty() {
            self.pending_commands.write().await.extend(all_commands);
        }

        Ok(())
    }

    async fn subscribe(&self, _handler: Box<dyn EventHandler>) -> Result<()> {
        Err(BusError::SubscribeNotSupported)
    }
}
