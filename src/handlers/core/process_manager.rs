//! Process Manager event handler.
//!
//! Receives events from the event bus and orchestrates process manager execution
//! using the shared orchestration module.
//!
//! Works with any `PMContextFactory` implementation — gRPC (distributed)
//! or local (standalone) — enabling deploy-anywhere process manager code.

use std::sync::Arc;

use backon::ExponentialBuilder;
use futures::future::BoxFuture;
use tokio::sync::Mutex;
use tracing::{debug, error, Instrument};

use crate::bus::{BusError, EventBus, EventHandler};
use crate::descriptor::Target;
use crate::orchestration::command::CommandExecutor;
use crate::orchestration::destination::DestinationFetcher;
use crate::orchestration::process_manager::grpc::GrpcPMContextFactory;
use crate::orchestration::process_manager::{orchestrate_pm, PMContextFactory};
use crate::proto::process_manager_service_client::ProcessManagerServiceClient;
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;
use crate::storage::EventStore;
use crate::utils::retry::saga_backoff;

/// Event handler that orchestrates process manager execution via a context factory.
///
/// Uses `PMContextFactory` to create per-invocation contexts, enabling
/// the same handler code for both distributed (gRPC) and standalone (local) modes.
pub struct ProcessManagerEventHandler {
    context_factory: Arc<dyn PMContextFactory>,
    destination_fetcher: Arc<dyn DestinationFetcher>,
    command_executor: Arc<dyn CommandExecutor>,
    /// Target filter — only handle events matching these targets.
    /// Empty means handle all events (distributed mode uses bus-level filtering).
    targets: Vec<Target>,
    backoff: ExponentialBuilder,
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
            targets: Vec::new(),
            backoff: saga_backoff(),
        }
    }

    /// Set target filter for handler-level event filtering.
    pub fn with_targets(mut self, targets: Vec<Target>) -> Self {
        self.targets = targets;
        self
    }

    /// Create with custom backoff configuration.
    pub fn with_backoff(mut self, backoff: ExponentialBuilder) -> Self {
        self.backoff = backoff;
        self
    }

    /// Create a new process manager event handler using gRPC client.
    ///
    /// PM state events are persisted directly to the event store and published
    /// to the event bus, bypassing the command pipeline.
    pub fn new(
        client: ProcessManagerServiceClient<tonic::transport::Channel>,
        process_domain: String,
        destination_fetcher: Arc<dyn DestinationFetcher>,
        command_executor: Arc<dyn CommandExecutor>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        let factory = Arc::new(GrpcPMContextFactory::new(
            Arc::new(Mutex::new(client)),
            event_store,
            event_bus,
            process_domain.clone(),
            process_domain,
        ));
        Self {
            context_factory: factory,
            destination_fetcher,
            command_executor,
            targets: Vec::new(),
            backoff: saga_backoff(),
        }
    }
}

impl EventHandler for ProcessManagerEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        // Check subscription filter (handler-level filtering for standalone mode)
        if !self.targets.is_empty() && !crate::bus::any_target_matches(&book, &self.targets) {
            return Box::pin(async { Ok(()) });
        }

        let correlation_id = book.correlation_id().to_string();
        let pm_name = self.context_factory.name().to_string();
        let pm_domain = self.context_factory.pm_domain().to_string();
        let span = tracing::info_span!("pm.handle", %pm_name, %correlation_id, %pm_domain);

        let factory = self.context_factory.clone();
        let destination_fetcher = self.destination_fetcher.clone();
        let command_executor = self.command_executor.clone();
        let backoff = self.backoff;

        Box::pin(
            async move {
                let book_owned = (*book).clone();

                tracing::info!(
                    pages = book_owned.pages.len(),
                    has_snapshot = book_owned.snapshot.is_some(),
                    domain = %book_owned.domain(),
                    "PM handler received book from bus"
                );

                if correlation_id.is_empty() {
                    debug!("Event has no correlation_id, skipping process manager");
                    return Ok(());
                }

                let ctx = factory.create();

                if let Err(e) = orchestrate_pm(
                    ctx.as_ref(),
                    destination_fetcher.as_ref(),
                    command_executor.as_ref(),
                    &book_owned,
                    &pm_name,
                    &pm_domain,
                    &correlation_id,
                    backoff,
                )
                .await
                {
                    error!(
                        error = %e,
                        "Process manager orchestration failed"
                    );
                    return Err(e);
                }

                Ok(())
            }
            .instrument(span),
        )
    }
}
