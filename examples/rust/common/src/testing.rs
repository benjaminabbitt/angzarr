//! Shared test utilities for aggregate handler tests.
//!
//! Provides common helpers to reduce boilerplate in test modules.

use angzarr::proto::{
    business_response, BusinessResponse, CommandBook, CommandPage, Cover, EventBook,
    Uuid as ProtoUuid,
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
