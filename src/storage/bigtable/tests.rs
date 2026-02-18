//! Unit tests for Bigtable storage implementations.
//!
//! These tests focus on pure functions (row key construction, data parsing)
//! that don't require a real Bigtable instance.

use uuid::Uuid;

use super::*;

// ============================================================================
// Row Key Tests
// ============================================================================

mod row_key_tests {
    use super::*;

    #[test]
    fn test_event_row_key_format() {
        let root = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let key = BigtableEventStore::row_key("orders", "main", root, 42);

        // Format: {domain}#{edition}#{root}#{sequence:010}
        assert_eq!(
            key,
            b"orders#main#12345678-1234-1234-1234-123456789abc#0000000042"
        );
    }

    #[test]
    fn test_event_row_key_sequence_padding() {
        let root = Uuid::nil();
        let key = BigtableEventStore::row_key("test", "v1", root, 0);
        assert!(key.ends_with(b"#0000000000"));

        let key = BigtableEventStore::row_key("test", "v1", root, 999999999);
        assert!(key.ends_with(b"#0999999999"));

        let key = BigtableEventStore::row_key("test", "v1", root, u32::MAX);
        assert!(key.ends_with(b"#4294967295"));
    }

    #[test]
    fn test_event_row_key_prefix() {
        let root = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let prefix = BigtableEventStore::row_key_prefix("inventory", "staging", root);

        assert_eq!(
            prefix,
            b"inventory#staging#aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee#"
        );
    }

    #[test]
    fn test_parse_event_row_key_valid() {
        let key = b"orders#main#12345678-1234-1234-1234-123456789abc#0000000042";
        let parsed = BigtableEventStore::parse_row_key(key);

        assert!(parsed.is_some());
        let (domain, edition, root, seq) = parsed.unwrap();
        assert_eq!(domain, "orders");
        assert_eq!(edition, "main");
        assert_eq!(
            root,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        assert_eq!(seq, 42);
    }

    #[test]
    fn test_parse_event_row_key_invalid_format() {
        // Too few parts
        assert!(BigtableEventStore::parse_row_key(b"orders#main#root").is_none());

        // Invalid UUID
        assert!(BigtableEventStore::parse_row_key(b"orders#main#not-a-uuid#0000000001").is_none());

        // Invalid sequence
        assert!(BigtableEventStore::parse_row_key(
            b"orders#main#12345678-1234-1234-1234-123456789abc#notanumber"
        )
        .is_none());
    }

    #[test]
    fn test_snapshot_row_key_format() {
        let root = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let key = BigtableSnapshotStore::row_key("orders", "main", root, 100);

        assert_eq!(
            String::from_utf8(key).unwrap(),
            "orders#main#12345678-1234-1234-1234-123456789abc#0000000100"
        );
    }

    #[test]
    fn test_position_row_key_format() {
        let root = [0xDE, 0xAD, 0xBE, 0xEF];
        let key = BigtablePositionStore::row_key("projector-orders", "orders", "main", &root);

        assert_eq!(
            String::from_utf8(key).unwrap(),
            "projector-orders#orders#main#deadbeef"
        );
    }

    #[test]
    fn test_position_row_key_empty_root() {
        let root: [u8; 0] = [];
        let key = BigtablePositionStore::row_key("handler", "domain", "edition", &root);

        assert_eq!(String::from_utf8(key).unwrap(), "handler#domain#edition#");
    }
}

// ============================================================================
// Sequence Extraction Tests
// ============================================================================

mod sequence_tests {
    use super::*;
    use crate::proto::event_page::Sequence;
    use crate::proto::EventPage;

    #[test]
    fn test_get_sequence_from_num() {
        let event = EventPage {
            sequence: Some(Sequence::Num(42)),
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(BigtableEventStore::get_sequence(&event), 42);
    }

    #[test]
    fn test_get_sequence_from_force() {
        let event = EventPage {
            sequence: Some(Sequence::Force(true)),
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(BigtableEventStore::get_sequence(&event), 0);
    }

    #[test]
    fn test_get_sequence_none() {
        let event = EventPage {
            sequence: None,
            event: None,
            created_at: None,
            external_payload: None,
        };
        assert_eq!(BigtableEventStore::get_sequence(&event), 0);
    }
}

// ============================================================================
// Timestamp Parsing Tests
// ============================================================================

mod timestamp_tests {
    use super::*;

    #[test]
    fn test_parse_iso8601_timestamp_valid() {
        let ts = BigtableEventStore::parse_timestamp("2024-01-15T10:30:00Z");
        assert!(ts.is_some());
        let (secs, nanos) = ts.unwrap();
        assert!(secs > 0);
        assert_eq!(nanos, 0);
    }

    #[test]
    fn test_parse_iso8601_timestamp_with_nanos() {
        let ts = BigtableEventStore::parse_timestamp("2024-01-15T10:30:00.123456789Z");
        assert!(ts.is_some());
        let (_, nanos) = ts.unwrap();
        assert!(nanos > 0);
    }

    #[test]
    fn test_parse_iso8601_timestamp_invalid() {
        assert!(BigtableEventStore::parse_timestamp("not-a-timestamp").is_none());
        assert!(BigtableEventStore::parse_timestamp("2024-13-45").is_none());
    }

    #[test]
    fn test_format_timestamp() {
        let formatted = BigtableEventStore::format_timestamp(1705315800, 0);
        assert!(formatted.contains("2024-01-15"));
    }
}

// ============================================================================
// Mutation Building Tests
// ============================================================================

mod mutation_tests {
    use super::*;
    use crate::proto::EventPage;

    #[test]
    fn test_build_set_cell_mutation() {
        let value = b"test_value";
        let mutation = BigtableEventStore::build_set_cell("cf", b"col", value);

        // Verify the mutation is properly constructed
        assert!(mutation.mutation.is_some());
    }

    #[test]
    fn test_build_event_mutations() {
        let event = EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(0)),
            event: Some(prost_types::Any {
                type_url: "test.Event".to_string(),
                value: vec![1, 2, 3],
            }),
            created_at: Some(prost_types::Timestamp {
                seconds: 1705315800,
                nanos: 0,
            }),
            external_payload: None,
        };

        let mutations = BigtableEventStore::build_event_mutations(&event, "corr-123");

        // Should have: data, created_at, correlation_id
        assert!(mutations.len() >= 2);
    }
}
