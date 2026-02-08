use super::*;
use crate::bus::MockEventBus;
use crate::proto::{CommandBook, CommandPage, Cover, RevocationResponse, Uuid as ProtoUuid};
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
            edition: None,
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
fn test_generate_correlation_id_empty_stays_empty() {
    let command = make_command_book(false);
    let result = generate_correlation_id(&command).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_extract_events_from_response_with_events() {
    let event_book = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
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
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
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
    };

    let result = publish_and_build_response(&event_bus, event_book).await;
    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::Internal);
}
