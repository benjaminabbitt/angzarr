//! Local (in-process) saga context.
//!
//! Implements `SagaRetryContext` by composing in-process `CommandExecutor`,
//! `DestinationFetcher`, and `SagaHandler`. One instance per saga invocation.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::error;

use crate::orchestration::command::{CommandExecutor, CommandOutcome};
use crate::orchestration::destination::DestinationFetcher;
use crate::proto::{CommandBook, Cover, EventBook};
use crate::standalone::SagaHandler;

use super::{SagaContextFactory, SagaRetryContext};

/// In-process saga context.
///
/// Delegates command execution and destination fetching to shared orchestration
/// traits. Saga prepare/execute calls go directly to the `SagaHandler` impl.
pub struct LocalSagaContext {
    command_executor: Arc<dyn CommandExecutor>,
    destination_fetcher: Arc<dyn DestinationFetcher>,
    saga_handler: Arc<dyn SagaHandler>,
    source: Arc<EventBook>,
}

impl LocalSagaContext {
    /// Create a new local saga context for one saga invocation.
    pub fn new(
        command_executor: Arc<dyn CommandExecutor>,
        destination_fetcher: Arc<dyn DestinationFetcher>,
        saga_handler: Arc<dyn SagaHandler>,
        source: Arc<EventBook>,
    ) -> Self {
        Self {
            command_executor,
            destination_fetcher,
            saga_handler,
            source,
        }
    }
}

#[async_trait]
impl SagaRetryContext for LocalSagaContext {
    async fn execute_command(&self, command: CommandBook) -> CommandOutcome {
        self.command_executor.execute(command).await
    }

    async fn prepare_destinations(
        &self,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        self.saga_handler
            .prepare(&self.source)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn fetch_destination(&self, cover: &Cover) -> Option<EventBook> {
        self.destination_fetcher.fetch(cover).await
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
/// Captures in-process saga handler and orchestration dependencies.
/// Each call to `create()` produces a context for one saga invocation.
pub struct LocalSagaContextFactory {
    command_executor: Arc<dyn CommandExecutor>,
    destination_fetcher: Arc<dyn DestinationFetcher>,
    saga_handler: Arc<dyn SagaHandler>,
}

impl LocalSagaContextFactory {
    /// Create a new factory with the saga handler and orchestration dependencies.
    pub fn new(
        command_executor: Arc<dyn CommandExecutor>,
        destination_fetcher: Arc<dyn DestinationFetcher>,
        saga_handler: Arc<dyn SagaHandler>,
    ) -> Self {
        Self {
            command_executor,
            destination_fetcher,
            saga_handler,
        }
    }
}

impl SagaContextFactory for LocalSagaContextFactory {
    fn create(&self, source: Arc<EventBook>) -> Box<dyn SagaRetryContext> {
        Box::new(LocalSagaContext::new(
            self.command_executor.clone(),
            self.destination_fetcher.clone(),
            self.saga_handler.clone(),
            source,
        ))
    }
}
