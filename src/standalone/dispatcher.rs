//! Command dispatcher for standalone runtime.
//!
//! Thin routing layer that dispatches commands to per-domain `AggregateCommandHandler`s.
//! This replaces the monolithic `CommandRouter` with a simpler dispatch-only approach.
//!
//! In standalone mode, handlers communicate via gRPC over UDS.
//! In distributed mode, handlers are in separate pods with TCP.

use std::collections::HashMap;
use std::sync::Arc;

use tonic::Status;
use tracing::info;

use crate::handlers::core::AggregateCommandHandler;
use crate::proto::{BusinessResponse, CommandBook, CommandResponse};
use crate::proto_ext::CoverExt;

/// Command dispatcher that routes commands to per-domain handlers.
///
/// No storage, no business logic — just dispatch by domain.
/// Each handler owns its own context factory and execution logic.
#[derive(Clone)]
pub struct CommandDispatcher {
    handlers: Arc<HashMap<String, Arc<AggregateCommandHandler>>>,
}

impl CommandDispatcher {
    /// Create a new dispatcher with the given handlers.
    pub fn new(handlers: HashMap<String, Arc<AggregateCommandHandler>>) -> Self {
        let domains: Vec<_> = handlers.keys().cloned().collect();
        info!(
            domains = ?domains,
            "Command dispatcher initialized"
        );

        Self {
            handlers: Arc::new(handlers),
        }
    }

    /// Get list of registered domains.
    pub fn domains(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Execute a command by routing to the appropriate domain handler.
    pub async fn execute(&self, command: CommandBook) -> Result<CommandResponse, Status> {
        let domain = command.domain();

        let handler = self.handlers.get(domain).ok_or_else(|| {
            Status::not_found(format!("No handler registered for domain: {domain}"))
        })?;

        handler.execute(command).await
    }

    /// Execute a compensation command (for saga/PM rejection handling).
    ///
    /// Routes to the domain's handler and extracts the BusinessResponse.
    pub async fn execute_compensation(
        &self,
        command: CommandBook,
    ) -> Result<BusinessResponse, Status> {
        let response = self.execute(command).await?;

        // Extract business response from command response
        // The events contain the compensation result
        Ok(BusinessResponse {
            result: Some(crate::proto::business_response::Result::Events(
                response.events.unwrap_or_default(),
            )),
        })
    }

    /// Get a handler for a specific domain (for direct access).
    pub fn get_handler(&self, domain: &str) -> Option<&Arc<AggregateCommandHandler>> {
        self.handlers.get(domain)
    }
}
