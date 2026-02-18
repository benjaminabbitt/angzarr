//! Extension traits for proto types.
//!
//! Provides convenient accessor methods for common patterns like extracting
//! domain, correlation_id, and root_id from Cover-bearing types.
//!
//! ## Module Organization
//!
//! - [`constants`] - Shared constants (domain names, type URLs, headers)
//! - [`cover`] - CoverExt trait for accessing cover fields
//! - [`edition`] - EditionExt trait and Edition constructors
//! - [`uuid`] - UUID conversion traits
//! - [`pages`] - EventPageExt and CommandPageExt traits
//! - [`books`] - EventBookExt, CommandBookExt, and sequence helpers
//! - [`grpc`] - gRPC utilities for correlation and tracing

pub mod books;
pub mod constants;
pub mod cover;
pub mod edition;
pub mod grpc;
pub mod pages;
pub mod uuid;

// Re-export all public items for convenient imports
pub use books::{calculate_next_sequence, calculate_set_next_seq, CommandBookExt, EventBookExt};
pub use constants::{
    COMPONENT_REGISTERED_TYPE_URL, CORRELATION_ID_HEADER, DEFAULT_EDITION, META_ANGZARR_DOMAIN,
    PROJECTION_DOMAIN_PREFIX, PROJECTION_TYPE_URL, REGISTER_COMPONENT_TYPE_URL, UNKNOWN_DOMAIN,
    WILDCARD_DOMAIN,
};
pub use cover::CoverExt;
pub use edition::EditionExt;
pub use grpc::correlated_request;
pub use pages::{CommandPageExt, EventPageExt};
pub use uuid::{ProtoUuidExt, UuidExt};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{
        CommandBook, Cover, Edition, EventBook, MergeStrategy, SnapshotRetention, Uuid as ProtoUuid,
    };

    fn make_cover(domain: &str, correlation_id: &str, root: Option<::uuid::Uuid>) -> Cover {
        Cover {
            domain: domain.to_string(),
            correlation_id: correlation_id.to_string(),
            root: root.map(|u| ProtoUuid {
                value: u.as_bytes().to_vec(),
            }),
            edition: None,
        }
    }

    #[test]
    fn test_event_book_with_cover() {
        let root = ::uuid::Uuid::new_v4();
        let book = EventBook {
            cover: Some(make_cover("orders", "corr-123", Some(root))),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        assert_eq!(book.domain(), "orders");
        assert_eq!(book.correlation_id(), "corr-123");
        assert!(book.has_correlation_id());
        assert_eq!(book.root_uuid(), Some(root));
        assert_eq!(book.root_id_hex(), Some(hex::encode(root.as_bytes())));
    }

    #[test]
    fn test_event_book_without_cover() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        assert_eq!(book.domain(), "unknown");
        assert_eq!(book.correlation_id(), "");
        assert!(!book.has_correlation_id());
        assert_eq!(book.root_uuid(), None);
        assert_eq!(book.root_id_hex(), None);
    }

    #[test]
    fn test_command_book_with_cover() {
        let book = CommandBook {
            cover: Some(make_cover("inventory", "corr-456", None)),
            pages: vec![],
            saga_origin: None,
        };

        assert_eq!(book.domain(), "inventory");
        assert_eq!(book.correlation_id(), "corr-456");
        assert!(book.has_correlation_id());
        assert_eq!(book.root_uuid(), None);
    }

    #[test]
    fn test_edition_main_timeline() {
        let edition = Edition::main_timeline();
        assert!(edition.is_main_timeline());
        assert_eq!(edition.name_or_default(), "angzarr");
    }

    #[test]
    fn test_edition_implicit() {
        let edition = Edition::implicit("v2");
        assert!(!edition.is_main_timeline());
        assert_eq!(edition.name, "v2");
        assert!(edition.divergences.is_empty());
    }

    #[test]
    fn test_edition_explicit_divergence() {
        let edition = Edition::explicit(
            "v2",
            vec![
                crate::proto::DomainDivergence {
                    domain: "order".to_string(),
                    sequence: 50,
                },
                crate::proto::DomainDivergence {
                    domain: "inventory".to_string(),
                    sequence: 75,
                },
            ],
        );
        assert_eq!(edition.divergence_for("order"), Some(50));
        assert_eq!(edition.divergence_for("inventory"), Some(75));
        assert_eq!(edition.divergence_for("other"), None);
    }

    #[test]
    fn test_edition_from_string() {
        let edition: Edition = "v2".into();
        assert_eq!(edition.name, "v2");
        assert!(edition.divergences.is_empty());
    }

    // EventPage tests
    #[test]
    fn test_event_page_sequence_num() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: Some(Sequence::Num(42)),
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.sequence_num(), 42);
    }

    #[test]
    fn test_event_page_sequence_num_force() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: Some(Sequence::Force(true)),
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.sequence_num(), 0);
    }

    #[test]
    fn test_event_page_sequence_num_none() {
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: None,
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.sequence_num(), 0);
    }

    #[test]
    fn test_event_page_type_url() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: Some(Sequence::Num(1)),
            event: Some(prost_types::Any {
                type_url: "type.googleapis.com/test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.type_url(), Some("type.googleapis.com/test.Event"));
    }

    #[test]
    fn test_event_page_type_url_none() {
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: None,
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.type_url(), None);
    }

    #[test]
    fn test_event_page_payload() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: Some(Sequence::Num(1)),
            event: Some(prost_types::Any {
                type_url: "test".to_string(),
                value: vec![1, 2, 3],
            }),
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.payload(), Some(&[1u8, 2, 3][..]));
    }

    #[test]
    fn test_event_page_payload_none() {
        use crate::proto::EventPage;

        let page = EventPage {
            sequence: None,
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(page.payload(), None);
    }

    #[test]
    fn test_event_page_decode() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;
        use prost::Message;

        let msg = prost_types::Duration {
            seconds: 99,
            nanos: 0,
        };
        let page = EventPage {
            sequence: Some(Sequence::Num(1)),
            event: Some(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.Duration".to_string(),
                value: msg.encode_to_vec(),
            }),
            created_at: None,
            external_payload: None,
        };
        let decoded: Option<prost_types::Duration> = page.decode("Duration");
        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap().seconds, 99);
    }

    #[test]
    fn test_event_page_decode_type_mismatch() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;
        use prost::Message;

        let msg = prost_types::Duration {
            seconds: 99,
            nanos: 0,
        };
        let page = EventPage {
            sequence: Some(Sequence::Num(1)),
            event: Some(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.Duration".to_string(),
                value: msg.encode_to_vec(),
            }),
            created_at: None,
            external_payload: None,
        };
        let decoded: Option<prost_types::Duration> = page.decode("Timestamp");
        assert!(decoded.is_none());
    }

    // CommandPage tests
    #[test]
    fn test_command_page_sequence_num() {
        use crate::proto::CommandPage;

        let page = CommandPage {
            sequence: 77,
            command: None,
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        };
        assert_eq!(page.sequence_num(), 77);
    }

    #[test]
    fn test_command_page_type_url() {
        use crate::proto::CommandPage;

        let page = CommandPage {
            sequence: 1,
            command: Some(prost_types::Any {
                type_url: "type.googleapis.com/test.Command".to_string(),
                value: vec![],
            }),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        };
        assert_eq!(page.type_url(), Some("type.googleapis.com/test.Command"));
    }

    #[test]
    fn test_command_page_type_url_none() {
        use crate::proto::CommandPage;

        let page = CommandPage {
            sequence: 1,
            command: None,
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        };
        assert_eq!(page.type_url(), None);
    }

    #[test]
    fn test_command_page_payload() {
        use crate::proto::CommandPage;

        let page = CommandPage {
            sequence: 1,
            command: Some(prost_types::Any {
                type_url: "test".to_string(),
                value: vec![4, 5, 6],
            }),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        };
        assert_eq!(page.payload(), Some(&[4u8, 5, 6][..]));
    }

    #[test]
    fn test_command_page_decode() {
        use crate::proto::CommandPage;
        use prost::Message;

        let msg = prost_types::Duration {
            seconds: 123,
            nanos: 0,
        };
        let page = CommandPage {
            sequence: 1,
            command: Some(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.Duration".to_string(),
                value: msg.encode_to_vec(),
            }),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        };
        let decoded: Option<prost_types::Duration> = page.decode("Duration");
        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap().seconds, 123);
    }

    // EventBook extension tests
    #[test]
    fn test_event_book_next_sequence() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            next_sequence: 42,
        };
        assert_eq!(book.next_sequence(), 42);
    }

    #[test]
    fn test_event_book_is_empty_true() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            next_sequence: 0,
        };
        assert!(book.is_empty());
    }

    #[test]
    fn test_event_book_is_empty_false() {
        use crate::proto::EventPage;

        let book = EventBook {
            cover: None,
            pages: vec![EventPage::default()],
            snapshot: None,
            next_sequence: 0,
        };
        assert!(!book.is_empty());
    }

    #[test]
    fn test_event_book_last_page() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let book = EventBook {
            cover: None,
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: None,
                    created_at: None,
                    external_payload: None,
                },
                EventPage {
                    sequence: Some(Sequence::Num(2)),
                    event: None,
                    created_at: None,
                    external_payload: None,
                },
            ],
            snapshot: None,
            next_sequence: 0,
        };
        let last = book.last_page().unwrap();
        assert_eq!(last.sequence_num(), 2);
    }

    #[test]
    fn test_event_book_first_page() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let book = EventBook {
            cover: None,
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: None,
                    created_at: None,
                    external_payload: None,
                },
                EventPage {
                    sequence: Some(Sequence::Num(2)),
                    event: None,
                    created_at: None,
                    external_payload: None,
                },
            ],
            snapshot: None,
            next_sequence: 0,
        };
        let first = book.first_page().unwrap();
        assert_eq!(first.sequence_num(), 1);
    }

    // calculate_next_sequence tests
    #[test]
    fn test_calculate_next_sequence_from_pages() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let pages = vec![
            EventPage {
                sequence: Some(Sequence::Num(5)),
                event: None,
                created_at: None,
                external_payload: None,
            },
            EventPage {
                sequence: Some(Sequence::Num(6)),
                event: None,
                created_at: None,
                external_payload: None,
            },
        ];
        assert_eq!(calculate_next_sequence(&pages, None), 7);
    }

    #[test]
    fn test_calculate_next_sequence_from_snapshot() {
        use crate::proto::Snapshot;

        let pages: Vec<crate::proto::EventPage> = vec![];
        let snapshot = Snapshot {
            sequence: 10,
            state: None,
            retention: SnapshotRetention::RetentionDefault as i32,
        };
        assert_eq!(calculate_next_sequence(&pages, Some(&snapshot)), 11);
    }

    #[test]
    fn test_calculate_next_sequence_empty() {
        let pages: Vec<crate::proto::EventPage> = vec![];
        assert_eq!(calculate_next_sequence(&pages, None), 0);
    }

    #[test]
    fn test_calculate_set_next_seq() {
        use crate::proto::event_page::Sequence;
        use crate::proto::EventPage;

        let mut book = EventBook {
            cover: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(10)),
                event: None,
                created_at: None,
                external_payload: None,
            }],
            snapshot: None,
            next_sequence: 0,
        };
        calculate_set_next_seq(&mut book);
        assert_eq!(book.next_sequence, 11);
    }

    // CommandBook extension tests
    #[test]
    fn test_command_book_command_sequence() {
        use crate::proto::CommandPage;

        let book = CommandBook {
            cover: None,
            pages: vec![CommandPage {
                sequence: 25,
                command: None,
                merge_strategy: MergeStrategy::MergeCommutative as i32,
                external_payload: None,
            }],
            saga_origin: None,
        };
        assert_eq!(book.command_sequence(), 25);
    }

    #[test]
    fn test_command_book_command_sequence_empty() {
        let book = CommandBook {
            cover: None,
            pages: vec![],
            saga_origin: None,
        };
        assert_eq!(book.command_sequence(), 0);
    }

    #[test]
    fn test_command_book_first_command() {
        use crate::proto::CommandPage;

        let book = CommandBook {
            cover: None,
            pages: vec![CommandPage {
                sequence: 1,
                command: None,
                merge_strategy: MergeStrategy::MergeCommutative as i32,
                external_payload: None,
            }],
            saga_origin: None,
        };
        assert!(book.first_command().is_some());
    }

    // UUID extension tests
    #[test]
    fn test_proto_uuid_to_uuid() {
        let uuid = ::uuid::Uuid::new_v4();
        let proto = ProtoUuid {
            value: uuid.as_bytes().to_vec(),
        };
        let back = proto.to_uuid().unwrap();
        assert_eq!(uuid, back);
    }

    #[test]
    fn test_proto_uuid_to_uuid_invalid() {
        let proto = ProtoUuid {
            value: vec![1, 2, 3], // invalid length
        };
        assert!(proto.to_uuid().is_err());
    }

    #[test]
    fn test_proto_uuid_to_hex() {
        let proto = ProtoUuid {
            value: vec![0x01, 0x02, 0x03, 0x04],
        };
        assert_eq!(proto.to_hex(), "01020304");
    }

    #[test]
    fn test_uuid_to_proto_uuid() {
        let uuid = ::uuid::Uuid::new_v4();
        let proto = uuid.to_proto_uuid();
        assert_eq!(proto.value, uuid.as_bytes().to_vec());
    }

    // Edition empty test
    #[test]
    fn test_edition_is_empty_true() {
        let edition = Edition {
            name: String::new(),
            divergences: vec![],
        };
        assert!(edition.is_empty());
    }

    #[test]
    fn test_edition_is_empty_false() {
        let edition = Edition {
            name: "test".to_string(),
            divergences: vec![],
        };
        assert!(!edition.is_empty());
    }

    #[test]
    fn test_edition_name_or_default() {
        let empty = Edition {
            name: String::new(),
            divergences: vec![],
        };
        assert_eq!(empty.name_or_default(), DEFAULT_EDITION);

        let named = Edition {
            name: "custom".to_string(),
            divergences: vec![],
        };
        assert_eq!(named.name_or_default(), "custom");
    }
}
