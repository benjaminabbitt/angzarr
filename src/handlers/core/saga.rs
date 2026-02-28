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

use crate::bus::{BusError, CommandBus, EventHandler};
use crate::orchestration::command::CommandExecutor;
use crate::orchestration::destination::DestinationFetcher;
use crate::orchestration::saga::{orchestrate_saga, OutputDomainValidator, SagaContextFactory};
use crate::orchestration::FactExecutor;
use crate::proto::{EventBook, SyncMode};
use crate::proto_ext::CoverExt;
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
    command_bus: Option<Arc<dyn CommandBus>>,
    destination_fetcher: Option<Arc<dyn DestinationFetcher>>,
    fact_executor: Option<Arc<dyn FactExecutor>>,
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
            command_bus: None,
            destination_fetcher,
            fact_executor: None,
            output_domain_validator: None,
            backoff: saga_backoff(),
        }
    }

    /// Create from a context factory with output domain validation and fact injection.
    pub fn from_factory_with_validator(
        context_factory: Arc<dyn SagaContextFactory>,
        command_executor: Arc<dyn CommandExecutor>,
        command_bus: Option<Arc<dyn CommandBus>>,
        destination_fetcher: Option<Arc<dyn DestinationFetcher>>,
        fact_executor: Option<Arc<dyn FactExecutor>>,
        output_domain_validator: Option<Arc<OutputDomainValidator>>,
        backoff: ExponentialBuilder,
    ) -> Self {
        Self {
            context_factory,
            command_executor,
            command_bus,
            destination_fetcher,
            fact_executor,
            output_domain_validator,
            backoff,
        }
    }
}

impl EventHandler for SagaEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let correlation_id = book.correlation_id().to_string();
        let saga_name = self.context_factory.name().to_string();
        let span = tracing::info_span!("saga.handle", %saga_name, %correlation_id);

        let factory = self.context_factory.clone();
        let executor = self.command_executor.clone();
        let command_bus = self.command_bus.clone();
        let fetcher = self.destination_fetcher.clone();
        let fact_executor = self.fact_executor.clone();
        let validator = self.output_domain_validator.clone();
        let backoff = self.backoff;

        Box::pin(
            async move {
                let ctx = factory.create(book);

                let validator_ref: Option<&OutputDomainValidator> = validator.as_deref();
                let command_bus_ref: Option<&dyn CommandBus> = command_bus.as_deref();
                let fetcher_ref: Option<&dyn DestinationFetcher> = fetcher.as_deref();
                let fact_executor_ref: Option<&dyn FactExecutor> = fact_executor.as_deref();

                // Events received from bus are always async mode.
                // CASCADE mode doesn't publish to bus, so sagas called via bus
                // execute their commands with standard async behavior.
                if let Err(e) = orchestrate_saga(
                    ctx.as_ref(),
                    executor.as_ref(),
                    command_bus_ref,
                    fetcher_ref,
                    fact_executor_ref,
                    &saga_name,
                    &correlation_id,
                    validator_ref,
                    SyncMode::Async,
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
            }
            .instrument(span),
        )
    }
}
