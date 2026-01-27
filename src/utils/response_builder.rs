//! Response building logic for AggregateService.
//!
//! Handles EventBook response building, event page construction,
//! and correlation ID propagation.

#![allow(clippy::result_large_err)]

use std::sync::Arc;

use tonic::{Response, Status};

use crate::bus::EventBus;
use crate::bus::PublishResult;
use crate::proto::{business_response, BusinessResponse, CommandBook, CommandResponse, EventBook};

/// Generates a correlation ID for a command if one is not already provided.
///
/// Uses UUIDv5 with the angzarr namespace to generate a deterministic but unique
/// ID based on the command content.
///
/// # Arguments
/// * `command_book` - The command book to generate a correlation ID for
///
/// # Returns
/// The existing correlation ID if present, otherwise a newly generated one.
pub fn generate_correlation_id(command_book: &CommandBook) -> Result<String, Status> {
    let existing = command_book
        .cover
        .as_ref()
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");

    if !existing.is_empty() {
        return Ok(existing.to_string());
    }

    use prost::Message;
    let mut buf = Vec::new();
    command_book.encode(&mut buf).map_err(|e| {
        Status::internal(format!("Failed to encode command for correlation ID: {e}"))
    })?;

    // Create angzarr namespace from DNS namespace
    let angzarr_ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev");
    Ok(uuid::Uuid::new_v5(&angzarr_ns, &buf).to_string())
}

/// Extracts events from a BusinessResponse, handling revocation and empty responses.
///
/// # Arguments
/// * `response` - The business logic response
/// * `correlation_id` - The correlation ID to propagate to events
///
/// # Returns
/// The EventBook from the response with correlation ID set, or an error for revocation.
pub fn extract_events_from_response(
    response: BusinessResponse,
    correlation_id: String,
) -> Result<EventBook, Status> {
    let mut events = match response.result {
        Some(business_response::Result::Events(events)) => events,
        Some(business_response::Result::Revocation(revocation)) => {
            // Business logic explicitly requested framework handling
            return Err(Status::failed_precondition(format!(
                "Command revoked: {}",
                revocation.reason
            )));
        }
        None => {
            // Empty response - return empty EventBook
            EventBook {
                cover: None,
                pages: vec![],
                snapshot: None,
                snapshot_state: None,
            }
        }
    };

    // Propagate correlation ID from command to events via cover
    if let Some(ref mut cover) = events.cover {
        if cover.correlation_id.is_empty() {
            cover.correlation_id = correlation_id;
        }
    }

    Ok(events)
}

/// Publishes events and builds the final CommandResponse.
///
/// # Arguments
/// * `event_bus` - The event bus to publish to
/// * `event_book` - The events to publish
///
/// # Returns
/// A CommandResponse containing the events and projection results.
pub async fn publish_and_build_response(
    event_bus: &Arc<dyn EventBus>,
    event_book: EventBook,
) -> Result<Response<CommandResponse>, Status> {
    // Wrap in Arc for immutable distribution
    let event_book = Arc::new(event_book);

    // Notify event bus (projectors, sagas)
    let publish_result = event_bus
        .publish(Arc::clone(&event_book))
        .await
        .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

    Ok(Response::new(build_command_response(
        event_book,
        publish_result,
    )))
}

/// Builds a CommandResponse from an EventBook and publish result.
///
/// # Arguments
/// * `event_book` - The events (wrapped in Arc)
/// * `publish_result` - The result from publishing to the event bus
///
/// # Returns
/// A CommandResponse with the events and projections.
pub fn build_command_response(
    event_book: Arc<EventBook>,
    publish_result: PublishResult,
) -> CommandResponse {
    CommandResponse {
        events: Some(Arc::try_unwrap(event_book).unwrap_or_else(|arc| (*arc).clone())),
        projections: publish_result.projections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MockEventBus;
    use crate::proto::{CommandPage, Cover, RevocationResponse, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_command_book(with_correlation: bool) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: if with_correlation {
                    "test-correlation-id".to_string()
                } else {
                    String::new()
                },
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(Any {
                    type_url: "test.Command".to_string(),
                    value: vec![],
                }),
            }],
            saga_origin: None,
        }
    }

    #[test]
    fn test_generate_correlation_id_existing() {
        let command = make_command_book(true);
        let result = generate_correlation_id(&command).unwrap();
        assert_eq!(result, "test-correlation-id");
    }

    #[test]
    fn test_generate_correlation_id_generated() {
        let command = make_command_book(false);
        let result = generate_correlation_id(&command).unwrap();

        // Should be a valid UUID
        assert!(!result.is_empty());
        assert!(uuid::Uuid::parse_str(&result).is_ok());
    }

    #[test]
    fn test_generate_correlation_id_deterministic() {
        let command1 = make_command_book(false);
        let command2 = command1.clone();

        let result1 = generate_correlation_id(&command1).unwrap();
        let result2 = generate_correlation_id(&command2).unwrap();

        // Same command should generate same correlation ID
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_extract_events_from_response_with_events() {
        let event_book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: None,
                correlation_id: String::new(),
            }),
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        };
        let response = BusinessResponse {
            result: Some(business_response::Result::Events(event_book)),
        };

        let result = extract_events_from_response(response, "test-correlation".to_string());
        assert!(result.is_ok());
        let events = result.unwrap();
        // Correlation ID should be set on cover
        assert_eq!(
            events.cover.as_ref().unwrap().correlation_id,
            "test-correlation"
        );
    }

    #[test]
    fn test_extract_events_from_response_revocation() {
        let response = BusinessResponse {
            result: Some(business_response::Result::Revocation(RevocationResponse {
                emit_system_revocation: false,
                send_to_dead_letter_queue: false,
                escalate: false,
                abort: false,
                reason: "insufficient funds".to_string(),
            })),
        };

        let result = extract_events_from_response(response, "test-correlation".to_string());
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert!(status.message().contains("insufficient funds"));
    }

    #[test]
    fn test_extract_events_from_response_empty() {
        let response = BusinessResponse { result: None };

        let result = extract_events_from_response(response, "test-correlation".to_string());
        assert!(result.is_ok());
        let events = result.unwrap();
        assert!(events.pages.is_empty());
        // No cover means no correlation ID - that's expected for empty responses
    }

    #[tokio::test]
    async fn test_publish_and_build_response_success() {
        let event_bus: Arc<dyn EventBus> = Arc::new(MockEventBus::new());
        let event_book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: None,
                correlation_id: "test-correlation".to_string(),
            }),
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        };

        let result = publish_and_build_response(&event_bus, event_book).await;
        assert!(result.is_ok());

        let response = result.unwrap().into_inner();
        assert!(response.events.is_some());
        let events = response.events.unwrap();
        assert_eq!(
            events.cover.as_ref().unwrap().correlation_id,
            "test-correlation"
        );
    }

    #[tokio::test]
    async fn test_publish_and_build_response_bus_failure() {
        let mock_bus = Arc::new(MockEventBus::new());
        mock_bus.set_fail_on_publish(true).await;
        let event_bus: Arc<dyn EventBus> = mock_bus;

        let event_book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        };

        let result = publish_and_build_response(&event_bus, event_book).await;
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }
}
