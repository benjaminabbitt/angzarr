//! Adapter wrappers for bridging gRPC traits to standalone traits.
//!
//! Domain crates implement `AggregateLogic` (gRPC-style, returns `BusinessResponse`).
//! Standalone runtime expects `AggregateHandler` (returns `EventBook`).
//! These adapters bridge the gap.

use angzarr::proto::{
    business_response, BusinessResponse, ContextualCommand, Cover, EventBook, SagaResponse,
};
use angzarr::standalone::{AggregateHandler, SagaHandler};
use async_trait::async_trait;
use common::{AggregateLogic, SagaLogic};
use tonic::Status;

// ============================================================================
// AggregateLogic → AggregateHandler adapter
// ============================================================================

/// Wraps an `AggregateLogic` implementation to work as a standalone `AggregateHandler`.
///
/// `AggregateLogic` returns `BusinessResponse` (oneof: Events | Revocation).
/// `AggregateHandler` returns `EventBook` directly.
/// This adapter extracts the EventBook from successful responses.
pub struct AggregateLogicAdapter<T> {
    logic: T,
}

impl<T> AggregateLogicAdapter<T> {
    pub fn new(logic: T) -> Self {
        Self { logic }
    }
}

#[async_trait]
impl<T: AggregateLogic + 'static> AggregateHandler for AggregateLogicAdapter<T> {
    async fn handle(&self, command: ContextualCommand) -> Result<EventBook, Status> {
        let response: BusinessResponse = self.logic.handle(command).await?;

        match response.result {
            Some(business_response::Result::Events(events)) => Ok(events),
            Some(business_response::Result::Revocation(rev)) => {
                let reason = if rev.reason.is_empty() {
                    "command revoked by business logic".to_string()
                } else {
                    rev.reason
                };
                Err(Status::failed_precondition(reason))
            }
            None => Ok(EventBook::default()),
        }
    }
}

// ============================================================================
// SagaLogic → SagaHandler adapter
// ============================================================================

/// Wraps a `SagaLogic` implementation to work as a standalone `SagaHandler`.
///
/// `SagaLogic` is synchronous and returns `Vec<Cover>` / `Vec<CommandBook>`.
/// `SagaHandler` is async and returns `Result<Vec<Cover>, Status>` / `Result<SagaResponse, Status>`.
pub struct SagaLogicAdapter<T> {
    logic: T,
}

impl<T> SagaLogicAdapter<T> {
    pub fn new(logic: T) -> Self {
        Self { logic }
    }
}

#[async_trait]
impl<T: SagaLogic + 'static> SagaHandler for SagaLogicAdapter<T> {
    async fn prepare(&self, source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(self.logic.prepare(source))
    }

    async fn execute(
        &self,
        source: &EventBook,
        destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        let commands = self.logic.execute(source, destinations);
        Ok(SagaResponse {
            commands,
            events: vec![],
        })
    }
}
