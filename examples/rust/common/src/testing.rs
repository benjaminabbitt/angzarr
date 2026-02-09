//! Shared test utilities for aggregate handler tests.
//!
//! Provides common helpers to reduce boilerplate in test modules.

use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, CommandPage, Cover,
    EventBook, EventPage, Uuid as ProtoUuid,
};

/// Build a CommandBook for testing.
pub fn make_test_command_book(
    domain: &str,
    root: &[u8],
    type_url: &str,
    value: Vec<u8>,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence: 0,
            command: Some(prost_types::Any {
                type_url: type_url.to_string(),
                value,
            }),
        }],
        saga_origin: None,
    }
}

/// Extract EventBook from a BusinessResponse.
///
/// Panics if the response does not contain events.
pub fn extract_response_events(response: BusinessResponse) -> EventBook {
    match response.result {
        Some(business_response::Result::Events(events)) => events,
        _ => panic!("Expected events in response"),
    }
}

/// Build an EventBook for saga/projector testing.
///
/// Generates a random root UUID and wraps the event data.
/// Use this for testing saga handlers where you need an input EventBook.
pub fn make_test_event_book(
    domain: &str,
    event_type: &str,
    event_data: Vec<u8>,
    correlation_id: &str,
) -> EventBook {
    let root = uuid::Uuid::new_v4();
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: correlation_id.to_string(),
            edition: None,
        }),
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(0)),
            event: Some(prost_types::Any {
                type_url: format!("type.examples/examples.{}", event_type),
                value: event_data,
            }),
            created_at: None,
        }],
        snapshot: None,
        ..Default::default()
    }
}

/// Build an EventBook with multiple events for state rebuild testing.
///
/// Takes a list of (type_url, event_data) pairs and builds pages with
/// sequential sequence numbers starting at 0.
pub fn make_multi_event_book(domain: &str, events: Vec<(&str, Vec<u8>)>) -> EventBook {
    let pages = events
        .into_iter()
        .enumerate()
        .map(|(i, (type_url, value))| EventPage {
            sequence: Some(Sequence::Num(i as u32)),
            event: Some(prost_types::Any {
                type_url: type_url.to_string(),
                value,
            }),
            created_at: None,
        })
        .collect();

    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: vec![1; 16],
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages,
        snapshot: None,
        ..Default::default()
    }
}
