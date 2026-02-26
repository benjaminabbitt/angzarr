//! Tests for fact event injection.
//!
//! Verifies that:
//! - Fact events (with FactSequence marker) can be submitted to the coordinator
//! - The coordinator assigns real sequence numbers at persistence time
//! - Downstream consumers receive events with valid sequences (not FactSequence)
//! - Idempotency checking via Cover.external_id prevents duplicate facts
//!
//! ## Implementation Required
//!
//! These tests verify the fact injection feature. The following must be implemented:
//!
//! 1. `RuntimeBuilder::route_facts_to_aggregate(bool)` - config option (default: true)
//! 2. `Runtime::inject_fact(EventBook)` - method to inject fact events
//! 3. Coordinator logic to:
//!    - Check idempotency via Cover.external_id
//!    - Route to aggregate for state update (if configured)
//!    - Assign next sequence number, replacing FactSequence
//!    - Persist and publish with real sequence

use crate::common::*;
use angzarr::proto::{event_page, FactSequence};

/// Helper to create a fact event book with FactSequence marker.
pub fn create_fact_event_book(
    domain: &str,
    root: Uuid,
    external_id: &str,
    source: &str,
    description: &str,
) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: Uuid::new_v4().to_string(),
            edition: None,
            external_id: external_id.to_string(),
        }),
        pages: vec![EventPage {
            // FactSequence marker instead of explicit sequence number
            sequence_type: Some(event_page::SequenceType::Fact(FactSequence {
                source: source.to_string(),
                description: description.to_string(),
            })),
            payload: Some(event_page::Payload::Event(Any {
                type_url: "test.FactEvent".to_string(),
                value: vec![42, 43, 44],
            })),
            created_at: None,
        }],
        snapshot: None,
        ..Default::default()
    }
}

/// Test that FactSequence events can be created with the new proto structure.
#[tokio::test]
async fn test_fact_event_proto_structure() {
    let domain = "fact-proto-test";
    let root = Uuid::new_v4();

    let fact = create_fact_event_book(
        domain,
        root,
        "stripe_pi_abc123",
        "stripe",
        "Payment confirmed by webhook",
    );

    // Verify the structure
    let cover = fact.cover.as_ref().expect("Cover should exist");
    assert_eq!(cover.external_id, "stripe_pi_abc123");
    assert_eq!(cover.domain, domain);

    let page = fact.pages.first().expect("Should have page");
    match &page.sequence_type {
        Some(event_page::SequenceType::Fact(fs)) => {
            assert_eq!(fs.source, "stripe");
            assert_eq!(fs.description, "Payment confirmed by webhook");
        }
        _ => panic!("Expected FactSequence marker"),
    }
}

/// Test that regular events have sequence numbers.
#[tokio::test]
async fn test_regular_event_has_sequence() {
    let domain = "regular-event-test";
    let root = Uuid::new_v4();

    let event = create_test_event_book(domain, root, 5);

    let page = event.pages.first().expect("Should have page");
    match &page.sequence_type {
        Some(event_page::SequenceType::Sequence(seq)) => {
            assert_eq!(*seq, 5);
        }
        _ => panic!("Expected regular sequence number"),
    }
}

/// Test that Cover.external_id defaults to empty for regular events.
#[tokio::test]
async fn test_regular_event_empty_external_id() {
    let domain = "regular-ext-id-test";
    let root = Uuid::new_v4();

    let event = create_test_event_book(domain, root, 0);
    let cover = event.cover.as_ref().expect("Cover should exist");

    assert!(
        cover.external_id.is_empty(),
        "Regular events should have empty external_id"
    );
}

// ============================================================================
// PENDING TESTS - Require implementation of inject_fact and coordinator logic
// ============================================================================
//
// The following tests are documented but commented out until the coordinator
// fact injection logic is implemented:
//
// #[tokio::test]
// async fn test_fact_event_gets_sequence_assigned() {
//     // 1. Create runtime with aggregate
//     // 2. Submit regular command to establish sequence 0
//     // 3. Inject fact event with FactSequence marker
//     // 4. Verify downstream receives event with real sequence 1 (not FactSequence)
// }
//
// #[tokio::test]
// async fn test_fact_event_idempotency() {
//     // 1. Inject fact with external_id "xyz"
//     // 2. Inject same fact again with same external_id
//     // 3. Verify only one event persisted (deduplicated)
// }
//
// #[tokio::test]
// async fn test_fact_routed_to_aggregate() {
//     // 1. Configure route_facts_to_aggregate = true
//     // 2. Inject fact event
//     // 3. Verify aggregate.handle() was called
// }
//
// #[tokio::test]
// async fn test_fact_bypass_aggregate() {
//     // 1. Configure route_facts_to_aggregate = false
//     // 2. Inject fact event
//     // 3. Verify aggregate.handle() was NOT called
//     // 4. Verify event still persisted with assigned sequence
// }
