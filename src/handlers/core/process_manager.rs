//! Process Manager event handler.
//!
//! Receives events from the event bus and orchestrates process manager execution
//! using the shared orchestration module.
//!
//! Works with any `PMContextFactory` implementation — gRPC (distributed)
//! or local (standalone) — enabling deploy-anywhere process manager code.

use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::bus::{BusError, EventHandler};
use crate::orchestration::command::CommandExecutor;
use crate::orchestration::destination::DestinationFetcher;
use crate::orchestration::process_manager::grpc::GrpcPMContextFactory;
use crate::orchestration::process_manager::{orchestrate_pm, PMContextFactory};
use crate::proto::process_manager_client::ProcessManagerClient;
use crate::proto::{EventBook, Subscription};
use crate::utils::retry::RetryConfig;

/// Event handler that orchestrates process manager execution via a context factory.
///
/// Uses `PMContextFactory` to create per-invocation contexts, enabling
/// the same handler code for both distributed (gRPC) and standalone (local) modes.
pub struct ProcessManagerEventHandler {
    context_factory: Arc<dyn PMContextFactory>,
    destination_fetcher: Arc<dyn DestinationFetcher>,
    command_executor: Arc<dyn CommandExecutor>,
    /// Subscription filter — only handle events matching these subscriptions.
    /// Empty means handle all events (distributed mode uses bus-level filtering).
    subscriptions: Vec<Subscription>,
    retry_config: RetryConfig,
}

impl ProcessManagerEventHandler {
    /// Create from a context factory with fetcher and executor.
    pub fn from_factory(
        context_factory: Arc<dyn PMContextFactory>,
        destination_fetcher: Arc<dyn DestinationFetcher>,
        command_executor: Arc<dyn CommandExecutor>,
    ) -> Self {
        Self {
            context_factory,
            destination_fetcher,
            command_executor,
            subscriptions: Vec::new(),
            retry_config: RetryConfig::for_saga_commands(),
        }
    }

    /// Set subscription filter for handler-level event filtering.
    pub fn with_subscriptions(mut self, subscriptions: Vec<Subscription>) -> Self {
        self.subscriptions = subscriptions;
        self
    }

    /// Create with custom retry configuration.
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    // --- Backward-compatible constructor for distributed sidecar binaries ---

    /// Create a new process manager event handler using gRPC client.
    pub fn new(
        client: ProcessManagerClient<tonic::transport::Channel>,
        process_domain: String,
        destination_fetcher: Arc<dyn DestinationFetcher>,
        command_executor: Arc<dyn CommandExecutor>,
    ) -> Self {
        let factory = Arc::new(GrpcPMContextFactory::new(
            Arc::new(Mutex::new(client)),
            command_executor.clone(),
            process_domain,
        ));
        Self {
            context_factory: factory,
            destination_fetcher,
            command_executor,
            subscriptions: Vec::new(),
            retry_config: RetryConfig::for_saga_commands(),
        }
    }
}

impl EventHandler for ProcessManagerEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        // Check subscription filter (handler-level filtering for standalone mode)
        if !self.subscriptions.is_empty()
            && !crate::bus::any_subscription_matches(&book, &self.subscriptions)
        {
            return Box::pin(async { Ok(()) });
        }

        let factory = self.context_factory.clone();
        let destination_fetcher = self.destination_fetcher.clone();
        let command_executor = self.command_executor.clone();
        let retry_config = self.retry_config.clone();

        Box::pin(async move {
            let book_owned = (*book).clone();
            let correlation_id = book_owned
                .cover
                .as_ref()
                .map(|c| c.correlation_id.clone())
                .unwrap_or_default();

            if correlation_id.is_empty() {
                debug!("Event has no correlation_id, skipping process manager");
                return Ok(());
            }

            let ctx = factory.create();
            let pm_domain = factory.pm_domain().to_string();

            if let Err(e) = orchestrate_pm(
                ctx.as_ref(),
                destination_fetcher.as_ref(),
                command_executor.as_ref(),
                &book_owned,
                &pm_domain,
                &correlation_id,
                &retry_config,
            )
            .await
            {
                error!(
                    correlation_id = %correlation_id,
                    error = %e,
                    "Process manager orchestration failed"
                );
                return Err(e);
            }

            Ok(())
        })
    }
}
