//! Tests for gRPC handler adapter utilities.
//!
//! gRPC handler adapters bridge between user-implemented handler traits and
//! the internal ClientLogic interface. This enables:
//! - In-process command handling without TCP overhead
//! - Unified routing for both commands and compensation notifications
//!
//! Why this matters: The command router doesn't know if a handler is in-process
//! or remote. Adapters provide a consistent interface, but must correctly detect
//! notification commands (for compensation) vs regular commands. If notification
//! detection fails, compensation events won't be emitted and sagas can't recover.
//!
//! Key behaviors verified:
//! - Notification detection via type_url suffix matching
//! - Graceful handling of missing/empty command structures
//! - NoOpClientLogic returns Unimplemented for unregistered domains

use prost::Message;

use super::*;
use crate::proto::{command_page, page_header, CommandBook, CommandPage, PageHeader};

// ============================================================================
// is_notification_command Tests
// ============================================================================

/// Type URL ending in "Notification" is detected as a notification command.
///
/// Notifications trigger compensation handling instead of normal command flow.
/// The suffix match allows any package prefix (angzarr, custom, etc).
#[test]
fn test_is_notification_command_with_notification_suffix() {
    let any = prost_types::Any {
        type_url: "type.googleapis.com/angzarr.Notification".to_string(),
        value: vec![],
    };
    assert!(is_notification_command(&any));
}

/// Any package prefix is accepted as long as type name is "Notification".
#[test]
fn test_is_notification_command_with_full_type_url() {
    let any = prost_types::Any {
        type_url: "some.other.package.Notification".to_string(),
        value: vec![],
    };
    assert!(is_notification_command(&any));
}

/// Regular commands (not ending in "Notification") go through normal handling.
#[test]
fn test_is_notification_command_with_regular_command() {
    let any = prost_types::Any {
        type_url: "type.googleapis.com/player.CreatePlayer".to_string(),
        value: vec![],
    };
    assert!(!is_notification_command(&any));
}

/// "Notification" must be a suffix, not just contained in the type URL.
///
/// A service named "NotificationService" should not trigger compensation flow.
#[test]
fn test_is_notification_command_with_notification_in_middle() {
    let any = prost_types::Any {
        type_url: "NotificationService.SendMessage".to_string(),
        value: vec![],
    };
    assert!(!is_notification_command(&any));
}

// ============================================================================
// decode_notification Tests
// ============================================================================

/// Valid protobuf bytes decode successfully into a Notification.
#[test]
fn test_decode_notification_valid() {
    let notification = Notification::default();
    let encoded = notification.encode_to_vec();
    let any = prost_types::Any {
        type_url: "angzarr.Notification".to_string(),
        value: encoded,
    };

    let result = decode_notification(&any);
    assert!(result.is_ok());
}

/// Invalid protobuf bytes return InvalidArgument status.
///
/// Corrupted messages should fail fast with a clear error rather than
/// causing undefined behavior downstream.
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

// ============================================================================
// extract_notification_from_command Tests
// ============================================================================

/// Helper to construct a ContextualCommand with a specific Any payload.
fn make_contextual_command_with_any(any: prost_types::Any) -> ContextualCommand {
    ContextualCommand {
        command: Some(CommandBook {
            cover: None,
            pages: vec![CommandPage {
                header: Some(PageHeader {
                    sequence_type: Some(page_header::SequenceType::Sequence(0)),
                }),
                payload: Some(command_page::Payload::Command(any)),
                merge_strategy: 0,
            }],
        }),
        events: None,
    }
}

/// Commands containing a Notification type are extracted for compensation handling.
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

/// Regular commands return None - they go through normal command handling.
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

/// Missing command book returns None, not an error.
///
/// This gracefully handles malformed requests without crashing.
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

/// Empty pages array returns None, not an error.
#[test]
fn test_extract_notification_from_command_with_empty_pages() {
    let cmd = ContextualCommand {
        command: Some(CommandBook {
            cover: None,
            pages: vec![],
        }),
        events: None,
    };

    let result = extract_notification_from_command(&cmd);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ============================================================================
// NoOpClientLogic Tests
// ============================================================================

/// NoOpClientLogic returns Unimplemented for invoke - domain has no handler.
///
/// This is used for fact injection into domains without aggregate handlers.
/// Commands fail, but facts (injected directly) can still be processed.
#[tokio::test]
async fn test_noop_client_logic_invoke_returns_unimplemented() {
    let noop = NoOpClientLogic;
    let cmd = ContextualCommand::default();

    let result = noop.invoke(cmd).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unimplemented);
}

/// NoOpClientLogic returns Unimplemented for replay - no handler to rebuild state.
#[tokio::test]
async fn test_noop_client_logic_replay_returns_unimplemented() {
    let noop = NoOpClientLogic;
    let events = EventBook::default();

    let result = noop.replay(&events).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unimplemented);
}
