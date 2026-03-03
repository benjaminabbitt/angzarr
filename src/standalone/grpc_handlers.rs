//! gRPC handler adapters for standalone mode.
//!
//! Bridges between handler traits and gRPC clients, enabling:
//! - In-process `CommandHandler` to be used as `ClientLogic` (no TCP bridge)

use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
use tonic::Status;
use tracing::instrument;

use crate::orchestration::aggregate::{ClientLogic, FactContext};
use crate::proto::business_response::Result as BusinessResult;
use crate::proto::{BusinessResponse, ContextualCommand, EventBook, Notification};

use super::traits::{CommandHandler, FactContext as TraitFactContext, ProcessManagerHandler};

pub use crate::orchestration::projector::GrpcProjectorHandler;

/// Type URL suffix for Notification.
const NOTIFICATION_SUFFIX: &str = "Notification";

/// Adapts an in-process `CommandHandler` as `ClientLogic`.
///
/// Eliminates the TCP bridge: calls the handler directly and wraps the
/// result in a `BusinessResponse`. Used by the standalone `Runtime` to avoid
/// spawning gRPC servers for Rust command handlers.
///
/// Detects `Notification` and routes to `handle_revocation()` for compensation.
pub struct CommandHandlerAdapter {
    handler: Arc<dyn CommandHandler>,
}

impl CommandHandlerAdapter {
    /// Wrap a command handler as a `ClientLogic` implementation.
    pub fn new(handler: Arc<dyn CommandHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl ClientLogic for CommandHandlerAdapter {
    #[instrument(name = "adapter.aggregate.invoke", skip_all)]
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        // Check for rejection notifications
        if let Some(notification) = extract_notification_from_command(&cmd)? {
            return Ok(self.handler.handle_revocation(&notification));
        }

        // Normal command handling
        let events = self.handler.handle(cmd).await?;
        Ok(BusinessResponse {
            result: Some(BusinessResult::Events(events)),
        })
    }

    async fn invoke_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
        // Convert orchestration FactContext to traits FactContext
        let trait_ctx = TraitFactContext {
            facts: ctx.facts,
            prior_events: ctx.prior_events,
        };
        self.handler.handle_fact(trait_ctx).await
    }

    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        self.handler.replay(events).await
    }
}

/// Adapts an in-process `ProcessManagerHandler` as `ClientLogic`.
///
/// PMs are aggregates — they receive compensation Notifications through
/// the standard command routing infrastructure. This adapter only handles
/// Notification commands (for compensation), not regular commands.
///
/// Flow:
/// 1. PM command rejected → saga_origin.triggering_aggregate = PM cover
/// 2. Notification command routes to PM domain
/// 3. CommandRouter invokes this adapter
/// 4. PM's handle_revocation() processes the Notification
/// 5. Compensation events returned to be persisted
pub struct ProcessManagerHandlerAdapter {
    handler: Arc<dyn ProcessManagerHandler>,
}

impl ProcessManagerHandlerAdapter {
    /// Wrap a process manager handler as a `ClientLogic` implementation.
    pub fn new(handler: Arc<dyn ProcessManagerHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl ClientLogic for ProcessManagerHandlerAdapter {
    #[instrument(name = "adapter.pm.invoke", skip_all)]
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        // PM only accepts Notification commands for compensation
        let command_book = cmd
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument(super::errmsg::MISSING_COMMAND))?;
        let page = command_book
            .pages
            .first()
            .ok_or_else(|| Status::invalid_argument(super::errmsg::EMPTY_COMMAND_PAGES))?;
        let command_any = match &page.payload {
            Some(crate::proto::command_page::Payload::Command(c)) => c,
            _ => {
                return Err(Status::invalid_argument(
                    super::errmsg::MISSING_COMMAND_PAYLOAD,
                ))
            }
        };

        if !is_notification_command(command_any) {
            return Err(Status::invalid_argument(
                "PM only accepts Notification commands for compensation",
            ));
        }

        // Decode Notification
        let notification = decode_notification(command_any)?;

        // PM state comes from cmd.events (loaded by CommandRouter)
        let pm_state = cmd.events.as_ref();

        // Call PM's revocation handler
        let (pm_events, revocation_response) =
            self.handler.handle_revocation(&notification, pm_state);

        // Return compensation events or revocation response
        match pm_events {
            Some(events) if !events.pages.is_empty() => Ok(BusinessResponse {
                result: Some(BusinessResult::Events(events)),
            }),
            _ => Ok(BusinessResponse {
                result: Some(BusinessResult::Revocation(revocation_response)),
            }),
        }
    }

    async fn replay(&self, _events: &EventBook) -> Result<prost_types::Any, Status> {
        // PMs don't support replay through this adapter
        // PM state is rebuilt via the normal PM flow
        Err(Status::unimplemented(
            "PM replay not supported through command adapter",
        ))
    }
}

// ============================================================================
// Pure Helper Functions (testable without infrastructure)
// ============================================================================

/// Check if a command Any is a Notification type.
fn is_notification_command(command_any: &prost_types::Any) -> bool {
    command_any.type_url.ends_with(NOTIFICATION_SUFFIX)
}

/// Decode a Notification from an Any.
#[allow(clippy::result_large_err)]
fn decode_notification(command_any: &prost_types::Any) -> Result<Notification, Status> {
    Notification::decode(command_any.value.as_slice())
        .map_err(|e| Status::invalid_argument(format!("Failed to decode Notification: {}", e)))
}

/// Extract the notification from a command if it is one.
///
/// Returns Ok(Some(notification)) if the command is a Notification,
/// Ok(None) if it's a normal command, or Err if the command structure is invalid.
#[allow(clippy::result_large_err)]
fn extract_notification_from_command(
    cmd: &ContextualCommand,
) -> Result<Option<Notification>, Status> {
    let command_book = match cmd.command.as_ref() {
        Some(book) => book,
        None => return Ok(None),
    };

    let page = match command_book.pages.first() {
        Some(p) => p,
        None => return Ok(None),
    };

    let command_any = match &page.payload {
        Some(crate::proto::command_page::Payload::Command(c)) => c,
        _ => return Ok(None),
    };

    if is_notification_command(command_any) {
        let notification = decode_notification(command_any)?;
        Ok(Some(notification))
    } else {
        Ok(None)
    }
}

/// No-op client logic for domains without registered handlers.
///
/// Used when injecting facts into a domain that doesn't have an aggregate
/// handler registered. Facts are passed through unchanged - the coordinator
/// assigns sequence numbers and persists/publishes them.
pub struct NoOpClientLogic;

#[async_trait]
impl ClientLogic for NoOpClientLogic {
    async fn invoke(&self, _cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        Err(Status::unimplemented(
            "No aggregate handler registered for this domain",
        ))
    }

    async fn replay(&self, _events: &EventBook) -> Result<prost_types::Any, Status> {
        Err(Status::unimplemented(
            "No aggregate handler registered for this domain",
        ))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{command_page, CommandBook, CommandPage};

    // ========================================================================
    // is_notification_command Tests
    // ========================================================================

    #[test]
    fn test_is_notification_command_with_notification_suffix() {
        let any = prost_types::Any {
            type_url: "type.googleapis.com/angzarr.Notification".to_string(),
            value: vec![],
        };
        assert!(is_notification_command(&any));
    }

    #[test]
    fn test_is_notification_command_with_full_type_url() {
        let any = prost_types::Any {
            type_url: "some.other.package.Notification".to_string(),
            value: vec![],
        };
        assert!(is_notification_command(&any));
    }

    #[test]
    fn test_is_notification_command_with_regular_command() {
        let any = prost_types::Any {
            type_url: "type.googleapis.com/player.CreatePlayer".to_string(),
            value: vec![],
        };
        assert!(!is_notification_command(&any));
    }

    #[test]
    fn test_is_notification_command_with_notification_in_middle() {
        // "Notification" must be a suffix, not just contained
        let any = prost_types::Any {
            type_url: "NotificationService.SendMessage".to_string(),
            value: vec![],
        };
        assert!(!is_notification_command(&any));
    }

    // ========================================================================
    // decode_notification Tests
    // ========================================================================

    #[test]
    fn test_decode_notification_valid() {
        // Notification has cover, payload, sent_at, metadata fields
        let notification = Notification::default();
        let encoded = notification.encode_to_vec();
        let any = prost_types::Any {
            type_url: "angzarr.Notification".to_string(),
            value: encoded,
        };

        let result = decode_notification(&any);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_notification_invalid_bytes() {
        let any = prost_types::Any {
            type_url: "angzarr.Notification".to_string(),
            value: vec![0xFF, 0xFF, 0xFF], // Invalid protobuf
        };

        let result = decode_notification(&any);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    // ========================================================================
    // extract_notification_from_command Tests
    // ========================================================================

    fn make_contextual_command_with_any(any: prost_types::Any) -> ContextualCommand {
        ContextualCommand {
            command: Some(CommandBook {
                cover: None,
                pages: vec![CommandPage {
                    sequence: 0,
                    payload: Some(command_page::Payload::Command(any)),
                    merge_strategy: 0,
                }],
                saga_origin: None,
            }),
            events: None,
        }
    }

    #[test]
    fn test_extract_notification_from_command_with_notification() {
        let notification = Notification::default();
        let any = prost_types::Any {
            type_url: "angzarr.Notification".to_string(),
            value: notification.encode_to_vec(),
        };
        let cmd = make_contextual_command_with_any(any);

        let result = extract_notification_from_command(&cmd);
        assert!(result.is_ok());
        let opt = result.unwrap();
        assert!(opt.is_some());
    }

    #[test]
    fn test_extract_notification_from_command_with_regular_command() {
        let any = prost_types::Any {
            type_url: "player.CreatePlayer".to_string(),
            value: vec![],
        };
        let cmd = make_contextual_command_with_any(any);

        let result = extract_notification_from_command(&cmd);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_extract_notification_from_command_with_no_command() {
        let cmd = ContextualCommand {
            command: None,
            events: None,
        };

        let result = extract_notification_from_command(&cmd);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_extract_notification_from_command_with_empty_pages() {
        let cmd = ContextualCommand {
            command: Some(CommandBook {
                cover: None,
                pages: vec![],
                saga_origin: None,
            }),
            events: None,
        };

        let result = extract_notification_from_command(&cmd);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ========================================================================
    // NoOpClientLogic Tests
    // ========================================================================

    #[tokio::test]
    async fn test_noop_client_logic_invoke_returns_unimplemented() {
        let noop = NoOpClientLogic;
        let cmd = ContextualCommand::default();

        let result = noop.invoke(cmd).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_noop_client_logic_replay_returns_unimplemented() {
        let noop = NoOpClientLogic;
        let events = EventBook::default();

        let result = noop.replay(&events).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unimplemented);
    }
}
