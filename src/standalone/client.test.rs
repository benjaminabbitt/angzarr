//! Tests for standalone in-process clients.
//!
//! Standalone clients provide the same interface as distributed gRPC clients
//! but route commands directly within the process. This enables:
//! - Unit testing without network overhead
//! - Local development with full functionality
//!
//! Why this matters: In-process clients must behave identically to gRPC clients.
//! Temporal parameter extraction must correctly parse as-of queries for speculative
//! execution. Edition deletion guards protect the main timeline from accidental
//! modification.
//!
//! Key behaviors verified:
//! - Temporal parameter extraction (sequence/timestamp for as-of queries)
//! - CommandBuilder field setting (domain, root, correlation_id, edition)
//! - Edition deletion guards (protect main timeline)

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use super::*;
use crate::bus::{ChannelConfig, ChannelEventBus};
use crate::discovery::StaticServiceDiscovery;
use crate::proto::command_page;
use crate::proto::temporal_query::PointInTime;
use crate::proto::TemporalQuery;
use crate::standalone::CommandRouter;
use crate::storage::MockPositionStore;

// ============================================================================
// Test Helpers
// ============================================================================

impl CommandBuilder {
    /// Test-only constructor that creates a minimal router.
    ///
    /// Enables unit testing of `build()` logic without full runtime setup.
    pub(crate) fn new_for_testing(domain: String, root: Uuid) -> Self {
        let router = Arc::new(CommandRouter::new(
            HashMap::new(),
            HashMap::new(),
            Arc::new(StaticServiceDiscovery::new()),
            Arc::new(ChannelEventBus::new(ChannelConfig::publisher())),
            vec![],
            vec![],
            vec![],
            None,
            Arc::new(MockPositionStore::new()),
        ));
        Self::new(router, domain, root)
    }
}

// ============================================================================
// extract_temporal_params Tests
// ============================================================================

/// No temporal query → (None, None).
///
/// When speculative execution has no temporal constraint,
/// use current state.
#[test]
fn test_extract_temporal_params_none_returns_none_none() {
    let result = extract_temporal_params(&None).unwrap();
    assert!(result.0.is_none(), "sequence should be None");
    assert!(result.1.is_none(), "timestamp should be None");
}

/// TemporalQuery present but no point_in_time → (None, None).
#[test]
fn test_extract_temporal_params_some_with_no_point_in_time_returns_none_none() {
    let temporal = TemporalQuery {
        point_in_time: None,
    };
    let result = extract_temporal_params(&Some(temporal)).unwrap();
    assert!(result.0.is_none(), "sequence should be None");
    assert!(result.1.is_none(), "timestamp should be None");
}

/// AsOfSequence extracts the sequence number.
///
/// Sequence-based temporal queries reconstruct state from events 0..=seq.
#[test]
fn test_extract_temporal_params_as_of_sequence_returns_some_sequence() {
    let temporal = TemporalQuery {
        point_in_time: Some(PointInTime::AsOfSequence(42)),
    };
    let result = extract_temporal_params(&Some(temporal)).unwrap();
    assert_eq!(result.0, Some(42), "sequence should be 42");
    assert!(result.1.is_none(), "timestamp should be None");
}

/// Sequence 0 is a valid temporal point (initial state before any events).
#[test]
fn test_extract_temporal_params_as_of_sequence_zero() {
    let temporal = TemporalQuery {
        point_in_time: Some(PointInTime::AsOfSequence(0)),
    };
    let result = extract_temporal_params(&Some(temporal)).unwrap();
    assert_eq!(result.0, Some(0), "sequence should be 0");
    assert!(result.1.is_none(), "timestamp should be None");
}

/// AsOfTime converts Timestamp to RFC3339 string.
///
/// Timestamp-based queries are useful for "what was state at this moment?"
#[test]
fn test_extract_temporal_params_as_of_time_returns_some_timestamp() {
    use prost_types::Timestamp;

    let ts = Timestamp {
        seconds: 1704067200, // 2024-01-01 00:00:00 UTC
        nanos: 0,
    };
    let temporal = TemporalQuery {
        point_in_time: Some(PointInTime::AsOfTime(ts)),
    };
    let result = extract_temporal_params(&Some(temporal)).unwrap();
    assert!(result.0.is_none(), "sequence should be None");
    assert!(result.1.is_some(), "timestamp should be Some");
    let timestamp = result.1.unwrap();
    assert!(
        timestamp.contains("2024-01-01"),
        "timestamp should contain date"
    );
}

// ============================================================================
// CommandBuilder Tests
// ============================================================================

/// CommandBuilder.build() sets domain in the Cover.
///
/// Domain routes the command to the correct aggregate handler.
#[test]
fn test_command_builder_build_sets_domain() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("test-domain".to_string(), root).build();

    let cover = command.cover.as_ref().unwrap();
    assert_eq!(cover.domain, "test-domain");
}

/// CommandBuilder.build() sets root ID in the Cover.
///
/// Root ID identifies the aggregate instance.
#[test]
fn test_command_builder_build_sets_root() {
    let root = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root).build();

    let cover = command.cover.as_ref().unwrap();
    let root_bytes = &cover.root.as_ref().unwrap().value;
    let parsed = Uuid::from_slice(root_bytes).unwrap();
    assert_eq!(parsed, root);
}

/// CommandBuilder.with_correlation_id() sets correlation ID for cross-domain tracing.
#[test]
fn test_command_builder_build_sets_correlation_id() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root)
        .with_correlation_id("corr-123")
        .build();

    let cover = command.cover.as_ref().unwrap();
    assert_eq!(cover.correlation_id, "corr-123");
}

/// CommandBuilder.with_edition() sets edition for diverged timeline targeting.
#[test]
fn test_command_builder_build_sets_edition() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root)
        .with_edition("test-edition")
        .build();

    let cover = command.cover.as_ref().unwrap();
    let edition = cover.edition.as_ref().unwrap();
    assert_eq!(edition.name, "test-edition");
}

/// CommandBuilder.with_type() and with_data() set command type URL and payload.
#[test]
fn test_command_builder_build_sets_command_type_and_data() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root)
        .with_type("CreateOrder")
        .with_data(vec![1, 2, 3, 4])
        .with_sequence(5)
        .build();

    let page = &command.pages[0];
    use crate::proto_ext::CommandPageExt;
    assert_eq!(page.sequence_num(), 5);
    if let Some(command_page::Payload::Command(any)) = &page.payload {
        assert_eq!(any.type_url, "CreateOrder");
        assert_eq!(any.value, vec![1, 2, 3, 4]);
    } else {
        panic!("Expected Command payload");
    }
}

/// CommandBuilder without with_type() uses empty type URL.
#[test]
fn test_command_builder_build_no_type_uses_empty() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root)
        .with_data(vec![1, 2, 3])
        .build();

    let page = &command.pages[0];
    if let Some(command_page::Payload::Command(any)) = &page.payload {
        assert_eq!(any.type_url, "");
    } else {
        panic!("Expected Command payload");
    }
}

/// CommandBuilder without data has no payload.
#[test]
fn test_command_builder_build_no_data_no_payload() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root)
        .with_type("CreateOrder")
        .build();

    let page = &command.pages[0];
    assert!(page.payload.is_none());
}

/// CommandBuilder.with_sequence() sets explicit sequence number.
#[test]
fn test_command_builder_build_with_sequence() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root)
        .with_sequence(42)
        .build();

    let page = &command.pages[0];
    use crate::proto_ext::CommandPageExt;
    assert_eq!(page.sequence_num(), 42);
}

/// CommandBuilder without sequence defaults to 0.
#[test]
fn test_command_builder_build_default_sequence_zero() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root).build();

    let page = &command.pages[0];
    use crate::proto_ext::CommandPageExt;
    assert_eq!(page.sequence_num(), 0);
}

/// CommandBuilder without correlation_id uses empty string.
#[test]
fn test_command_builder_build_default_empty_correlation_id() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root).build();

    let cover = command.cover.as_ref().unwrap();
    assert_eq!(cover.correlation_id, "");
}

/// CommandBuilder without edition has None edition.
#[test]
fn test_command_builder_build_default_no_edition() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root).build();

    let cover = command.cover.as_ref().unwrap();
    assert!(cover.edition.is_none());
}

/// CommandBuilder uses COMMUTATIVE merge strategy.
#[test]
fn test_command_builder_build_uses_commutative_merge() {
    let root = Uuid::new_v4();
    let command = CommandBuilder::new_for_testing("orders".to_string(), root).build();

    let page = &command.pages[0];
    assert_eq!(page.merge_strategy, MergeStrategy::MergeCommutative as i32);
}

// ============================================================================
// StandaloneQueryClient Tests
// ============================================================================

/// QueryClient stores domain storage map on construction.
#[test]
fn test_query_client_new_stores_domain_stores() {
    let stores = HashMap::new();
    let client = StandaloneQueryClient::new(stores);
    assert!(client.domain_stores.is_empty());
}

/// Empty edition string is rejected (main timeline protection).
///
/// Empty string is equivalent to main timeline—cannot delete.
#[tokio::test]
async fn test_delete_edition_events_rejects_empty_edition() {
    let client = StandaloneQueryClient::new(HashMap::new());
    let result = client.delete_edition_events("test-domain", "").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// Main timeline edition name is rejected.
///
/// "angzarr" is the canonical main timeline name—immutable by design.
#[tokio::test]
async fn test_delete_edition_events_rejects_main_timeline() {
    let client = StandaloneQueryClient::new(HashMap::new());
    let result = client
        .delete_edition_events("test-domain", DEFAULT_EDITION)
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// Unknown domain returns NotFound error.
#[tokio::test]
async fn test_delete_edition_events_rejects_unknown_domain() {
    let client = StandaloneQueryClient::new(HashMap::new());
    let result = client
        .delete_edition_events("unknown-domain", "test-edition")
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
}
