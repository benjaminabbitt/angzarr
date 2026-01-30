//! Local (in-process) saga context.
//!
//! Implements `SagaRetryContext` by composing in-process `CommandExecutor`,
//! `DestinationFetcher`, and `SagaHandler`. One instance per saga invocation.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::error;

use crate::proto::{CommandBook, Cover, EventBook};
use crate::standalone::SagaHandler;

use super::{SagaContextFactory, SagaRetryContext};

/// In-process saga context.
///
/// Saga prepare/execute calls go directly to the `SagaHandler` impl.
/// Command execution and destination fetching are handled externally by the caller.
pub struct LocalSagaContext {
    saga_handler: Arc<dyn SagaHandler>,
    source: Arc<EventBook>,
}

impl LocalSagaContext {
    /// Create a new local saga context for one saga invocation.
    pub fn new(saga_handler: Arc<dyn SagaHandler>, source: Arc<EventBook>) -> Self {
        Self {
            saga_handler,
            source,
        }
    }
}

#[async_trait]
impl SagaRetryContext for LocalSagaContext {
    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        self.saga_handler
            .prepare(&self.source)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn re_execute_saga(
        &self,
        destinations: Vec<EventBook>,
    ) -> Result<Vec<CommandBook>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .saga_handler
            .execute(&self.source, &destinations)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(response.commands)
    }

    async fn on_command_rejected(&self, _command: &CommandBook, reason: &str) {
        error!(reason = %reason, "Saga command permanently rejected");
    }
}

/// Factory that produces `LocalSagaContext` instances for standalone mode.
///
/// Captures in-process saga handler. Command execution and destination
/// fetching are handled by the event handler, not the factory.
pub struct LocalSagaContextFactory {
    saga_handler: Arc<dyn SagaHandler>,
    name: String,
}

impl LocalSagaContextFactory {
    /// Create a new factory with the saga handler and name.
    pub fn new(saga_handler: Arc<dyn SagaHandler>, name: String) -> Self {
        Self {
            saga_handler,
            name,
        }
    }
}

impl SagaContextFactory for LocalSagaContextFactory {
    fn create(&self, source: Arc<EventBook>) -> Box<dyn SagaRetryContext> {
        Box::new(LocalSagaContext::new(self.saga_handler.clone(), source))
    }

    fn name(&self) -> &str {
        &self.name
    }
}
