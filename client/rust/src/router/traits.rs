//! Handler traits for each component type.
//!
//! Each trait defines the contract for domain handlers. Implementors
//! encapsulate their routing logic internally and declare which types
//! they handle via `command_types()` or `event_types()`.

use prost_types::Any;
use std::error::Error;
use tonic::Status;

use crate::proto::{CommandBook, Cover, EventBook, Notification, Projection};
use crate::router::StateRouter;

// ============================================================================
// Common Types
// ============================================================================

/// Error type for command/event rejection with a human-readable reason.
#[derive(Debug, Clone)]
pub struct CommandRejectedError {
    pub reason: String,
}

impl CommandRejectedError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for CommandRejectedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command rejected: {}", self.reason)
    }
}

impl std::error::Error for CommandRejectedError {}

impl From<CommandRejectedError> for Status {
    fn from(err: CommandRejectedError) -> Self {
        Status::failed_precondition(err.reason)
    }
}

/// Result type for handlers.
pub type CommandResult<T> = std::result::Result<T, CommandRejectedError>;

/// Response from rejection handlers.
///
/// Handlers may return:
/// - Events to compensate/fix state
/// - Notification to forward upstream
/// - Both
#[derive(Default)]
pub struct RejectionHandlerResponse {
    /// Events to persist (compensation).
    pub events: Option<EventBook>,
    /// Notification to forward upstream.
    pub notification: Option<Notification>,
}

/// Response from saga handlers.
#[derive(Default)]
pub struct SagaHandlerResponse {
    /// Commands to send to other aggregates.
    pub commands: Vec<CommandBook>,
    /// Facts/events to inject to other aggregates.
    pub events: Vec<EventBook>,
}

/// Response from process manager handlers.
#[derive(Default)]
pub struct ProcessManagerResponse {
    /// Commands to send to other aggregates.
    pub commands: Vec<CommandBook>,
    /// Events to persist to the PM's own domain.
    pub process_events: Option<EventBook>,
    /// Facts to inject to other aggregates.
    pub facts: Vec<EventBook>,
}

/// Helper trait for unpacking Any messages.
pub trait UnpackAny {
    /// Unpack an Any to a specific message type.
    fn unpack<M: prost::Message + Default>(&self) -> Result<M, prost::DecodeError>;
}

impl UnpackAny for Any {
    fn unpack<M: prost::Message + Default>(&self) -> Result<M, prost::DecodeError> {
        M::decode(self.value.as_slice())
    }
}

// ============================================================================
// Command Handler
// ============================================================================

/// Handler for a single domain's command handler logic.
///
/// Command handlers receive commands and emit events. They maintain state
/// that is rebuilt from events using a `StateRouter`.
///
/// # Example
///
/// ```rust,ignore
/// struct PlayerHandler {
///     state_router: StateRouter<PlayerState>,
/// }
///
/// impl CommandHandlerDomainHandler for PlayerHandler {
///     type State = PlayerState;
///
///     fn command_types(&self) -> Vec<String> {
///         vec!["RegisterPlayer".into(), "DepositFunds".into()]
///     }
///
///     fn state_router(&self) -> &StateRouter<Self::State> {
///         &self.state_router
///     }
///
///     fn handle(
///         &self,
///         cmd: &CommandBook,
///         payload: &Any,
///         state: &Self::State,
///         seq: u32,
///     ) -> CommandResult<EventBook> {
///         dispatch_command!(payload, cmd, state, seq, {
///             "RegisterPlayer" => self.handle_register,
///             "DepositFunds" => self.handle_deposit,
///         })
///     }
/// }
/// ```
pub trait CommandHandlerDomainHandler: Send + Sync {
    /// The state type for this aggregate.
    type State: Default + 'static;

    /// Command type suffixes this handler processes.
    ///
    /// Used for subscription derivation and routing.
    fn command_types(&self) -> Vec<String>;

    /// Get the state router for rebuilding state from events.
    fn state_router(&self) -> &StateRouter<Self::State>;

    /// Rebuild state from events.
    ///
    /// Default implementation uses `state_router().with_event_book()`.
    fn rebuild(&self, events: &EventBook) -> Self::State {
        self.state_router().with_event_book(events)
    }

    /// Handle a command and return resulting events.
    ///
    /// The handler should dispatch internally based on `payload.type_url`.
    fn handle(
        &self,
        cmd: &CommandBook,
        payload: &Any,
        state: &Self::State,
        seq: u32,
    ) -> CommandResult<EventBook>;

    /// Handle a rejection notification.
    ///
    /// Called when a command issued by a saga/PM targeting this aggregate's
    /// domain was rejected. Override to provide custom compensation logic.
    ///
    /// Default implementation returns an empty response (framework handles).
    fn on_rejected(
        &self,
        _notification: &Notification,
        _state: &Self::State,
        _target_domain: &str,
        _target_command: &str,
    ) -> CommandResult<RejectionHandlerResponse> {
        Ok(RejectionHandlerResponse::default())
    }
}

// ============================================================================
// Saga Handler
// ============================================================================

/// Handler for a single domain's events in a saga.
///
/// Sagas are **pure translators**: they receive source events and produce
/// commands for target domains. They do NOT receive destination state —
/// the framework handles sequence stamping and delivery retries.
///
/// # Example
///
/// ```rust,ignore
/// struct OrderSagaHandler;
///
/// impl SagaDomainHandler for OrderSagaHandler {
///     fn event_types(&self) -> Vec<String> {
///         vec!["OrderCompleted".into(), "OrderCancelled".into()]
///     }
///
///     fn handle(
///         &self,
///         source: &EventBook,
///         event: &Any,
///     ) -> CommandResult<SagaHandlerResponse> {
///         dispatch_event!(event, source, {
///             "OrderCompleted" => self.handle_completed,
///             "OrderCancelled" => self.handle_cancelled,
///         })
///     }
/// }
/// ```
pub trait SagaDomainHandler: Send + Sync {
    /// Event type suffixes this handler processes.
    ///
    /// Used for subscription derivation.
    fn event_types(&self) -> Vec<String>;

    /// Translate source events into commands for target domains.
    ///
    /// Commands should have `cover` set to identify the target aggregate.
    /// The framework will stamp `angzarr_deferred` with source info and
    /// handle sequence assignment on delivery.
    ///
    /// Returns commands to send to other aggregates and events/facts to inject.
    fn handle(&self, source: &EventBook, event: &Any) -> CommandResult<SagaHandlerResponse>;

    /// Handle a rejection notification.
    ///
    /// Called when a saga-issued command was rejected. Override to provide
    /// custom compensation logic.
    ///
    /// Default implementation returns an empty response (framework handles).
    fn on_rejected(
        &self,
        _notification: &Notification,
        _target_domain: &str,
        _target_command: &str,
    ) -> CommandResult<RejectionHandlerResponse> {
        Ok(RejectionHandlerResponse::default())
    }
}

// ============================================================================
// Process Manager Handler
// ============================================================================

/// Handler for a single domain's events in a process manager.
///
/// Process managers correlate events across multiple domains and maintain
/// their own state. Each domain gets its own handler, but they all share
/// the same PM state type.
///
/// # Example
///
/// ```rust,ignore
/// struct OrderPmHandler;
///
/// impl ProcessManagerDomainHandler<HandFlowState> for OrderPmHandler {
///     fn event_types(&self) -> Vec<String> {
///         vec!["OrderCreated".into()]
///     }
///
///     fn prepare(&self, trigger: &EventBook, state: &HandFlowState, event: &Any) -> Vec<Cover> {
///         // Declare needed destinations
///         vec![]
///     }
///
///     fn handle(
///         &self,
///         trigger: &EventBook,
///         state: &HandFlowState,
///         event: &Any,
///         destinations: &[EventBook],
///     ) -> CommandResult<ProcessManagerResponse> {
///         // Process event, emit commands and/or PM events
///         Ok(ProcessManagerResponse::default())
///     }
/// }
/// ```
pub trait ProcessManagerDomainHandler<S>: Send + Sync {
    /// Event type suffixes this handler processes.
    fn event_types(&self) -> Vec<String>;

    /// Prepare phase — declare destination covers needed.
    fn prepare(&self, trigger: &EventBook, state: &S, event: &Any) -> Vec<Cover>;

    /// Handle phase — produce commands and PM events.
    fn handle(
        &self,
        trigger: &EventBook,
        state: &S,
        event: &Any,
        destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse>;

    /// Handle a rejection notification.
    ///
    /// Called when a PM-issued command was rejected. Override to provide
    /// custom compensation logic.
    fn on_rejected(
        &self,
        _notification: &Notification,
        _state: &S,
        _target_domain: &str,
        _target_command: &str,
    ) -> CommandResult<RejectionHandlerResponse> {
        Ok(RejectionHandlerResponse::default())
    }
}

// ============================================================================
// Projector Handler
// ============================================================================

/// Handler for a single domain's events in a projector.
///
/// Projectors consume events and produce external output (read models,
/// caches, external systems).
///
/// # Example
///
/// ```rust,ignore
/// struct PlayerProjectorHandler;
///
/// impl ProjectorDomainHandler for PlayerProjectorHandler {
///     fn event_types(&self) -> Vec<String> {
///         vec!["PlayerRegistered".into(), "FundsDeposited".into()]
///     }
///
///     fn project(&self, events: &EventBook) -> Result<Projection, Box<dyn Error + Send + Sync>> {
///         // Update external read model
///         Ok(Projection::default())
///     }
/// }
/// ```
pub trait ProjectorDomainHandler: Send + Sync {
    /// Event type suffixes this handler processes.
    fn event_types(&self) -> Vec<String>;

    /// Project events to external output.
    fn project(&self, events: &EventBook) -> Result<Projection, Box<dyn Error + Send + Sync>>;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_rejected_error_display() {
        let err = CommandRejectedError::new("insufficient funds");
        assert_eq!(err.to_string(), "Command rejected: insufficient funds");
    }

    #[test]
    fn command_rejected_error_to_status() {
        let err = CommandRejectedError::new("invalid input");
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[test]
    fn rejection_handler_response_default() {
        let response = RejectionHandlerResponse::default();
        assert!(response.events.is_none());
        assert!(response.notification.is_none());
    }

    #[test]
    fn process_manager_response_default() {
        let response = ProcessManagerResponse::default();
        assert!(response.commands.is_empty());
        assert!(response.process_events.is_none());
        assert!(response.facts.is_empty());
    }

    #[test]
    fn saga_handler_response_default() {
        let response = SagaHandlerResponse::default();
        assert!(response.commands.is_empty());
        assert!(response.events.is_empty());
    }
}
