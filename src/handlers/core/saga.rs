//! Saga event handler.
//!
//! Receives events from the event bus and orchestrates saga execution
//! using the shared orchestration module.
//!
//! Works with any `SagaContextFactory` implementation — gRPC (distributed)
//! or local (standalone) — enabling deploy-anywhere saga code.
//!
//! Supports:
//! - Two-phase saga protocol (prepare → fetch destinations → execute)
//! - Retry with backoff on sequence conflicts
//! - Output domain validation
//! - Compensation flow for rejected commands (via gRPC factory)

use std::sync::Arc;

use backon::ExponentialBuilder;
use futures::future::BoxFuture;
use tracing::{error, Instrument};

use crate::bus::{BusError, EventHandler};
use crate::orchestration::command::CommandExecutor;
use crate::orchestration::destination::DestinationFetcher;
use crate::orchestration::saga::{orchestrate_saga, OutputDomainValidator, SagaContextFactory};
use crate::proto::EventBook;
use crate::utils::retry::saga_backoff;

/// Event handler that orchestrates saga execution via a context factory.
///
/// Uses `SagaContextFactory` to create per-invocation contexts, enabling
/// the same handler code for both distributed (gRPC) and standalone (local) modes.
/// Command execution and destination fetching are passed directly to
/// orchestration functions, matching the PM handler pattern.
pub struct SagaEventHandler {
    context_factory: Arc<dyn SagaContextFactory>,
    command_executor: Arc<dyn CommandExecutor>,
    destination_fetcher: Option<Arc<dyn DestinationFetcher>>,
    output_domain_validator: Option<Arc<OutputDomainValidator>>,
    backoff: ExponentialBuilder,
}

impl SagaEventHandler {
    /// Create from a context factory with executor and fetcher.
    pub fn from_factory(
        context_factory: Arc<dyn SagaContextFactory>,
        command_executor: Arc<dyn CommandExecutor>,
        destination_fetcher: Option<Arc<dyn DestinationFetcher>>,
    ) -> Self {
        Self {
            context_factory,
            command_executor,
            destination_fetcher,
            output_domain_validator: None,
            backoff: saga_backoff(),
        }
    }

    /// Create from a context factory with output domain validation.
    pub fn from_factory_with_validator(
        context_factory: Arc<dyn SagaContextFactory>,
        command_executor: Arc<dyn CommandExecutor>,
        destination_fetcher: Option<Arc<dyn DestinationFetcher>>,
        output_domain_validator: Option<Arc<OutputDomainValidator>>,
        backoff: ExponentialBuilder,
    ) -> Self {
        Self {
            context_factory,
            command_executor,
            destination_fetcher,
            output_domain_validator,
            backoff,
        }
    }
}

impl EventHandler for SagaEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let correlation_id = book
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();
        let saga_name = self.context_factory.name().to_string();
        let span = tracing::info_span!("saga.handle", %saga_name, %correlation_id);

        let factory = self.context_factory.clone();
        let executor = self.command_executor.clone();
        let fetcher = self.destination_fetcher.clone();
        let validator = self.output_domain_validator.clone();
        let backoff = self.backoff;

        Box::pin(async move {
            let ctx = factory.create(book);

            let validator_ref: Option<&OutputDomainValidator> = validator.as_deref();
            let fetcher_ref: Option<&dyn DestinationFetcher> = fetcher.as_deref();

            if let Err(e) = orchestrate_saga(
                ctx.as_ref(),
                executor.as_ref(),
                fetcher_ref,
                &saga_name,
                &correlation_id,
                validator_ref,
                backoff,
            )
            .await
            {
                error!(
                    error = %e,
                    "Saga orchestration failed"
                );
            }

            Ok(())
        }.instrument(span))
    }
}
