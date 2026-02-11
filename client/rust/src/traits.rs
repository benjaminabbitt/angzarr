//! Client traits for gateway and query operations.
//!
//! These traits define the client API for interacting with angzarr services.
//! Both standalone (in-process) and distributed (gRPC) modes implement
//! the same traits, enabling deploy-anywhere client code.

use async_trait::async_trait;

use crate::error::Result;
use crate::proto::{
    CommandBook, CommandResponse, DryRunRequest, EventBook, ProcessManagerHandleResponse,
    Projection, Query, SagaResponse, SpeculatePmRequest, SpeculateProjectorRequest,
    SpeculateSagaRequest,
};

/// Trait for gateway client operations (command execution).
///
/// Implement this trait to create mock clients for testing or
/// alternative transport implementations.
#[async_trait]
pub trait GatewayClient: Send + Sync {
    /// Execute a command asynchronously (fire and forget).
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse>;
}

/// Trait for speculative execution operations.
///
/// Supports "what-if" scenarios: executing commands, projectors, sagas,
/// and process managers without persisting side effects.
#[async_trait]
pub trait SpeculativeClient: Send + Sync {
    /// Execute a command with dry-run (no persistence).
    async fn dry_run(&self, request: DryRunRequest) -> Result<CommandResponse>;

    /// Speculatively execute a projector against events.
    async fn projector(&self, request: SpeculateProjectorRequest) -> Result<Projection>;

    /// Speculatively execute a saga against events.
    async fn saga(&self, request: SpeculateSagaRequest) -> Result<SagaResponse>;

    /// Speculatively execute a process manager against events.
    async fn process_manager(
        &self,
        request: SpeculatePmRequest,
    ) -> Result<ProcessManagerHandleResponse>;
}

/// Trait for event query client operations.
///
/// Implement this trait to create mock clients for testing or
/// alternative transport implementations.
#[async_trait]
pub trait QueryClient: Send + Sync {
    /// Get a single EventBook for the given query.
    async fn get_event_book(&self, query: Query) -> Result<EventBook>;

    /// Get events as a vector (collects streaming results).
    async fn get_events(&self, query: Query) -> Result<Vec<EventBook>>;
}
