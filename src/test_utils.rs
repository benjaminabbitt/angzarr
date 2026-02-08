#![allow(dead_code)]
//! Shared test fixture builders for angzarr unit tests.
//!
//! Provides reusable constructors for proto types that appear across many test modules.

use crate::proto::{
    event_page, CommandBook, CommandPage, Cover, EventBook, EventPage, Uuid as ProtoUuid,
};
use prost_types::Any;
use uuid::Uuid;

/// Create a `ProtoUuid` from a random v4 UUID.
pub fn random_proto_uuid() -> ProtoUuid {
    proto_uuid(Uuid::new_v4())
}

/// Create a `ProtoUuid` from a specific `Uuid`.
pub fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

/// Create a `Cover` with the given domain and a random root UUID.
pub fn make_cover(domain: &str) -> Cover {
    make_cover_with_root(domain, Uuid::new_v4())
}

/// Create a `Cover` with the given domain and specific root UUID.
pub fn make_cover_with_root(domain: &str, root: Uuid) -> Cover {
    Cover {
        domain: domain.to_string(),
        root: Some(proto_uuid(root)),
        correlation_id: String::new(),
        edition: None,
    }
}

/// Create a `Cover` with domain, root, and correlation ID.
pub fn make_cover_full(domain: &str, root: Uuid, correlation_id: &str) -> Cover {
    Cover {
        domain: domain.to_string(),
        root: Some(proto_uuid(root)),
        correlation_id: correlation_id.to_string(),
        edition: None,
    }
}

/// Create an `EventPage` with a sequence number and a test type_url.
pub fn make_event_page(seq: u32) -> EventPage {
    EventPage {
        sequence: Some(event_page::Sequence::Num(seq)),
        event: Some(Any {
            type_url: format!("test.Event{}", seq),
            value: vec![],
        }),
        created_at: None,
    }
}

/// Create an `EventPage` with a specific type_url.
pub fn make_event_page_typed(seq: u32, type_url: &str) -> EventPage {
    EventPage {
        sequence: Some(event_page::Sequence::Num(seq)),
        event: Some(Any {
            type_url: type_url.to_string(),
            value: vec![],
        }),
        created_at: None,
    }
}

/// Create an `EventBook` with domain, random root, and provided pages.
pub fn make_event_book(domain: &str, pages: Vec<EventPage>) -> EventBook {
    EventBook {
        cover: Some(make_cover(domain)),
        pages,
        snapshot: None,
    }
}

/// Create an `EventBook` with domain, specific root, and provided pages.
pub fn make_event_book_with_root(domain: &str, root: Uuid, pages: Vec<EventPage>) -> EventBook {
    EventBook {
        cover: Some(make_cover_with_root(domain, root)),
        pages,
        snapshot: None,
    }
}

/// Create an empty `EventBook` for the given domain.
pub fn make_empty_event_book(domain: &str) -> EventBook {
    make_event_book(domain, vec![])
}

/// Create a `CommandBook` with domain, specific root, and a test command.
pub fn make_command_book(domain: &str, root: Uuid) -> CommandBook {
    make_command_book_with_sequence(domain, root, 0)
}

/// Create a `CommandBook` with domain, root, and specific sequence.
pub fn make_command_book_with_sequence(domain: &str, root: Uuid, sequence: u32) -> CommandBook {
    CommandBook {
        cover: Some(make_cover_with_root(domain, root)),
        pages: vec![CommandPage {
            sequence,
            command: Some(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            }),
        }],
        saga_origin: None,
    }
}

/// Create a `CommandBook` with optional correlation ID.
pub fn make_command_book_correlated(with_correlation: bool) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(random_proto_uuid()),
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
