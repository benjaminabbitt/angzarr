//! Local (in-process) saga context.
//!
//! Implements `SagaRetryContext` by composing in-process `CommandExecutor`,
//! `DestinationFetcher`, and `SagaHandler`. One instance per saga invocation.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, info, warn};

use crate::bus::EventBus;
use crate::config::SagaCompensationConfig;
use crate::proto::{CommandBook, EventBook, SagaResponse};
use crate::proto_ext::CoverExt;
use crate::standalone::CommandRouter;
use crate::standalone::SagaHandler;
use crate::utils::box_err;
use crate::utils::saga_compensation::{
    build_notification_command_book, process_compensation_response, CompensationContext,
};

use super::{SagaContextFactory, SagaRetryContext};

/// In-process saga context.
///
/// Saga prepare/execute calls go directly to the `SagaHandler` impl.
/// Command execution and destination fetching are handled externally by the caller.
/// Compensation flow uses the CommandRouter's execute_compensation method.
pub struct LocalSagaContext {
    saga_handler: Arc<dyn SagaHandler>,
    source: Arc<EventBook>,
    /// Router for compensation commands (None = no compensation support)
    router: Option<Arc<CommandRouter>>,
    /// Event bus for escalation handler
    event_bus: Option<Arc<dyn EventBus>>,
    /// Compensation configuration
    compensation_config: SagaCompensationConfig,
}

impl LocalSagaContext {
    /// Create a new local saga context for one saga invocation (no compensation).
    pub fn new(saga_handler: Arc<dyn SagaHandler>, source: Arc<EventBook>) -> Self {
        Self {
            saga_handler,
            source,
            router: None,
            event_bus: None,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create a new local saga context with compensation support.
    pub fn with_compensation(
        saga_handler: Arc<dyn SagaHandler>,
        source: Arc<EventBook>,
        router: Arc<CommandRouter>,
        event_bus: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
    ) -> Self {
        Self {
            saga_handler,
            source,
            router: Some(router),
            event_bus: Some(event_bus),
            compensation_config,
        }
    }
}

#[async_trait]
impl SagaRetryContext for LocalSagaContext {
    async fn handle(&self) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        let edition = self.source.edition().to_string();
        let mut response = self
            .saga_handler
            .handle(&self.source)
            .await
            .map_err(box_err)?;

        // Stamp edition on commands
        for cmd in &mut response.commands {
            if let Some(c) = &mut cmd.cover {
                c.stamp_edition_if_empty(&edition);
            }
        }
        Ok(response)
    }

    fn source_cover(&self) -> Option<&crate::proto::Cover> {
        self.source.cover.as_ref()
    }

    fn source_max_sequence(&self) -> u32 {
        use crate::proto_ext::EventPageExt;
        self.source
            .pages
            .iter()
            .map(|p| p.sequence_num())
            .max()
            .unwrap_or(0)
    }

    async fn on_command_rejected(&self, command: &CommandBook, reason: &str) {
        let (Some(router), Some(event_bus)) = (&self.router, &self.event_bus) else {
            error!(reason = %reason, "Saga command permanently rejected (no compensation path)");
            return;
        };

        let Some(context) = CompensationContext::from_rejected_command(command, reason.to_string())
        else {
            error!(reason = %reason, "Command rejected (not a saga command, no compensation)");
            return;
        };

        let source_domain = context
            .source
            .source
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("?");
        let target_domain = command
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        warn!(
            source_domain = %source_domain,
            source_seq = context.source.source_seq,
            target_domain = %target_domain,
            reason = %reason,
            "Saga command rejected, initiating compensation"
        );

        let notification_command = match build_notification_command_book(&context) {
            Ok(cmd) => cmd,
            Err(e) => {
                error!(
                    source_domain = %source_domain,
                    error = %e,
                    "Failed to build notification"
                );
                return;
            }
        };

        let triggering_domain = notification_command.domain().to_string();

        info!(
            source_domain = %source_domain,
            triggering_domain = %triggering_domain,
            "Sending rejection Notification to triggering aggregate via execute_compensation"
        );

        // Use router's execute_compensation to get BusinessResponse
        let response = router.execute_compensation(notification_command).await;

        // Process the BusinessResponse through shared handler
        process_compensation_response(
            response.map_err(|s| tonic::Status::internal(s.message())),
            &context,
            &self.compensation_config,
            event_bus,
            source_domain,
            &triggering_domain,
        )
        .await;
    }
}

/// Factory that produces `LocalSagaContext` instances for standalone mode.
///
/// Captures in-process saga handler and optional compensation dependencies.
/// Command execution and destination fetching are handled by the event handler.
pub struct LocalSagaContextFactory {
    saga_handler: Arc<dyn SagaHandler>,
    name: String,
    /// Router for compensation commands (None = no compensation support)
    router: Option<Arc<CommandRouter>>,
    /// Event bus for escalation handler
    event_bus: Option<Arc<dyn EventBus>>,
    /// Compensation configuration
    compensation_config: SagaCompensationConfig,
}

impl LocalSagaContextFactory {
    /// Create a new factory with the saga handler and name (no compensation).
    pub fn new(saga_handler: Arc<dyn SagaHandler>, name: String) -> Self {
        Self {
            saga_handler,
            name,
            router: None,
            event_bus: None,
            compensation_config: SagaCompensationConfig::default(),
        }
    }

    /// Create a new factory with compensation support.
    pub fn with_compensation(
        saga_handler: Arc<dyn SagaHandler>,
        name: String,
        router: Arc<CommandRouter>,
        event_bus: Arc<dyn EventBus>,
        compensation_config: SagaCompensationConfig,
    ) -> Self {
        Self {
            saga_handler,
            name,
            router: Some(router),
            event_bus: Some(event_bus),
            compensation_config,
        }
    }
}

impl SagaContextFactory for LocalSagaContextFactory {
    fn create(&self, source: Arc<EventBook>) -> Box<dyn SagaRetryContext> {
        match (&self.router, &self.event_bus) {
            (Some(router), Some(event_bus)) => Box::new(LocalSagaContext::with_compensation(
                self.saga_handler.clone(),
                source,
                router.clone(),
                event_bus.clone(),
                self.compensation_config.clone(),
            )),
            _ => Box::new(LocalSagaContext::new(self.saga_handler.clone(), source)),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
