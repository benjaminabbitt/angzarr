//! Tests for speculative execution.
//!
//! Speculative execution runs handler logic without side effects:
//! - No events persisted to event stores
//! - No commands executed against aggregates
//! - No messages published to event bus
//!
//! Why this matters: Speculative execution enables "what-if" queries, previewing
//! command effects before committing, and testing handler logic in isolation.
//! The same handler instances are reused—only framework behavior changes.
//!
//! Key behaviors verified:
//! - DomainStateSpec variants (Current, AtSequence, AtTimestamp, Explicit)
//! - PmSpeculativeResult structure holds commands, events, and facts
//! - EventBook construction helpers produce valid structures
//! - Domain-based routing finds correct projectors, sagas, and PMs

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use uuid::Uuid;

use super::*;
use crate::proto::event_page;
use crate::proto::{page_header, PageHeader};
use crate::proto_ext::EventPageExt;

// ============================================================================
// DomainStateSpec Tests
// ============================================================================

/// Current spec requests latest aggregate state.
#[test]
fn test_domain_state_spec_current() {
    let spec = DomainStateSpec::Current;
    assert!(format!("{:?}", spec).contains("Current"));
}

/// AtSequence spec requests state up to a specific event number.
#[test]
fn test_domain_state_spec_at_sequence() {
    let spec = DomainStateSpec::AtSequence(42);
    assert!(format!("{:?}", spec).contains("42"));
}

/// AtTimestamp spec requests state as of a point in time.
#[test]
fn test_domain_state_spec_at_timestamp() {
    let ts = "2024-01-15T10:30:00Z".to_string();
    let spec = DomainStateSpec::AtTimestamp(ts.clone());
    assert!(format!("{:?}", spec).contains(&ts));
}

/// Explicit spec uses caller-provided EventBook directly.
#[test]
fn test_domain_state_spec_explicit() {
    let book = EventBook::default();
    let spec = DomainStateSpec::Explicit(book);
    assert!(format!("{:?}", spec).contains("Explicit"));
}

/// DomainStateSpec must be Clone for use in HashMap values.
#[test]
fn test_domain_state_spec_clone() {
    let spec = DomainStateSpec::AtSequence(10);
    let cloned = spec.clone();
    assert!(format!("{:?}", cloned).contains("10"));
}

// ============================================================================
// PmSpeculativeResult Tests
// ============================================================================

/// Empty result has no commands, events, or facts.
#[test]
fn test_pm_speculative_result_empty() {
    let result = PmSpeculativeResult {
        commands: vec![],
        process_events: None,
        facts: vec![],
    };

    assert!(result.commands.is_empty());
    assert!(result.process_events.is_none());
    assert!(result.facts.is_empty());
}

/// Result can hold commands the PM would issue.
#[test]
fn test_pm_speculative_result_with_commands() {
    let cmd = CommandBook::default();
    let result = PmSpeculativeResult {
        commands: vec![cmd],
        process_events: None,
        facts: vec![],
    };

    assert_eq!(result.commands.len(), 1);
}

/// Result can hold PM events that would be persisted.
#[test]
fn test_pm_speculative_result_with_events() {
    let events = EventBook::default();
    let result = PmSpeculativeResult {
        commands: vec![],
        process_events: Some(events),
        facts: vec![],
    };

    assert!(result.process_events.is_some());
}

/// Result can hold facts the PM would inject.
#[test]
fn test_pm_speculative_result_with_facts() {
    let fact = EventBook::default();
    let result = PmSpeculativeResult {
        commands: vec![],
        process_events: None,
        facts: vec![fact],
    };

    assert_eq!(result.facts.len(), 1);
}

// ============================================================================
// SpeculativeExecutor::build_event_book Tests
// ============================================================================

/// build_event_book creates valid EventBook with cover.
#[test]
fn test_build_event_book_structure() {
    let root = Uuid::new_v4();
    let book = SpeculativeExecutor::build_event_book("order", root, vec![]);

    let cover = book.cover.as_ref().unwrap();
    assert_eq!(cover.domain, "order");
    assert_eq!(
        Uuid::from_slice(&cover.root.as_ref().unwrap().value).unwrap(),
        root
    );
    assert!(book.pages.is_empty());
    assert!(book.snapshot.is_none());
}

/// Pages are included in the built EventBook.
#[test]
fn test_build_event_book_with_pages() {
    let root = Uuid::new_v4();
    let page = EventPage {
        header: Some(PageHeader {
            sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(0)),
        }),
        payload: Some(event_page::Payload::Event(prost_types::Any {
            type_url: "test.Event".to_string(),
            value: vec![1, 2, 3],
        })),
        created_at: None,
    };
    let book = SpeculativeExecutor::build_event_book("order", root, vec![page]);

    assert_eq!(book.pages.len(), 1);
    assert_eq!(book.pages[0].sequence_num(), 0);
}

/// Multiple pages are preserved in order.
#[test]
fn test_build_event_book_multiple_pages() {
    let root = Uuid::new_v4();
    let pages: Vec<EventPage> = (0..5)
        .map(|seq| EventPage {
            header: Some(PageHeader {
                sequence_type: Some(page_header::SequenceType::Sequence(seq)),
            }),
            payload: Some(event_page::Payload::Event(prost_types::Any {
                type_url: format!("test.Event{}", seq),
                value: vec![],
            })),
            created_at: None,
        })
        .collect();

    let book = SpeculativeExecutor::build_event_book("order", root, pages);

    assert_eq!(book.pages.len(), 5);
    for (i, page) in book.pages.iter().enumerate() {
        assert_eq!(page.sequence_num(), i as u32);
    }
}

/// Built EventBook has default edition.
#[test]
fn test_build_event_book_has_default_edition() {
    let root = Uuid::new_v4();
    let book = SpeculativeExecutor::build_event_book("order", root, vec![]);

    let cover = book.cover.as_ref().unwrap();
    let edition = cover.edition.as_ref().unwrap();
    assert_eq!(edition.name, DEFAULT_EDITION);
    assert!(edition.divergences.is_empty());
}

/// Built EventBook has empty correlation_id.
#[test]
fn test_build_event_book_empty_correlation_id() {
    let root = Uuid::new_v4();
    let book = SpeculativeExecutor::build_event_book("order", root, vec![]);

    let cover = book.cover.as_ref().unwrap();
    assert!(cover.correlation_id.is_empty());
}

// ============================================================================
// SpeculativeExecutor::root_from_event_book Tests
// ============================================================================

/// Extracts root UUID from valid EventBook.
#[test]
fn test_root_from_event_book_valid() {
    let root = Uuid::new_v4();
    let book = SpeculativeExecutor::build_event_book("order", root, vec![]);
    assert_eq!(SpeculativeExecutor::root_from_event_book(&book), Some(root));
}

/// Returns None when cover is missing.
#[test]
fn test_root_from_event_book_missing_cover() {
    let book = EventBook::default();
    assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
}

/// Returns None when root is missing from cover.
#[test]
fn test_root_from_event_book_missing_root() {
    let book = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };
    assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
}

/// Returns None when UUID bytes are invalid (not 16 bytes).
#[test]
fn test_root_from_event_book_invalid_uuid_bytes() {
    let book = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid - not 16 bytes
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };
    assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
}

/// Page order is preserved (not sorted by sequence).
#[test]
fn test_build_event_book_preserves_page_order() {
    let root = Uuid::new_v4();
    let pages: Vec<EventPage> = vec![
        EventPage {
            header: Some(PageHeader {
                sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(5)),
            }),
            payload: None,
            created_at: None,
        },
        EventPage {
            header: Some(PageHeader {
                sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(3)),
            }),
            payload: None,
            created_at: None,
        },
        EventPage {
            header: Some(PageHeader {
                sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(7)),
            }),
            payload: None,
            created_at: None,
        },
    ];

    let book = SpeculativeExecutor::build_event_book("order", root, pages);

    assert_eq!(book.pages[0].sequence_num(), 5);
    assert_eq!(book.pages[1].sequence_num(), 3);
    assert_eq!(book.pages[2].sequence_num(), 7);
}

// ============================================================================
// Domain Routing Tests
// ============================================================================

mod domain_routing {
    use super::*;
    use crate::descriptor::Target;
    use crate::orchestration::projector::ProjectionMode;
    use crate::proto::{Cover, Projection, SagaResponse};
    use crate::standalone::traits::{ProcessManagerHandleResult, ProcessManagerHandler};

    /// Mock projector that always returns an empty projection.
    struct MockProjector;

    #[async_trait]
    impl super::super::ProjectorHandler for MockProjector {
        async fn handle(
            &self,
            _events: &EventBook,
            _mode: ProjectionMode,
        ) -> Result<Projection, Status> {
            Ok(Projection::default())
        }
    }

    /// Mock saga that returns empty response.
    struct MockSaga;

    #[async_trait]
    impl super::super::SagaHandler for MockSaga {
        async fn handle(&self, _source: &EventBook) -> Result<SagaResponse, Status> {
            Ok(SagaResponse::default())
        }
    }

    /// Mock process manager that returns empty result.
    struct MockPM;

    impl ProcessManagerHandler for MockPM {
        fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
            vec![]
        }

        fn handle(
            &self,
            _trigger: &EventBook,
            _process_state: Option<&EventBook>,
            _destinations: &[EventBook],
        ) -> ProcessManagerHandleResult {
            ProcessManagerHandleResult {
                commands: vec![],
                process_events: None,
                facts: vec![],
            }
        }
    }

    // ------------------------------------------------------------------------
    // Projector Domain Routing
    // ------------------------------------------------------------------------

    /// Projector is found when domain matches subscription.
    #[tokio::test]
    async fn test_projector_routing_finds_by_matching_domain() {
        let mut projectors = HashMap::new();
        projectors.insert(
            "orders-projector".to_string(),
            (
                Arc::new(MockProjector) as Arc<dyn super::super::ProjectorHandler>,
                vec!["orders".to_string()],
            ),
        );

        let executor =
            SpeculativeExecutor::new(projectors, HashMap::new(), HashMap::new(), HashMap::new());

        let events = EventBook::default();
        let result = executor
            .speculate_projector_by_domain("orders", &events)
            .await;

        assert!(result.is_ok());
    }

    /// Empty domains list means projector handles all domains.
    #[tokio::test]
    async fn test_projector_routing_empty_domains_matches_all() {
        let mut projectors = HashMap::new();
        projectors.insert(
            "catch-all-projector".to_string(),
            (
                Arc::new(MockProjector) as Arc<dyn super::super::ProjectorHandler>,
                vec![], // Empty = matches all domains
            ),
        );

        let executor =
            SpeculativeExecutor::new(projectors, HashMap::new(), HashMap::new(), HashMap::new());

        let events = EventBook::default();

        assert!(executor
            .speculate_projector_by_domain("orders", &events)
            .await
            .is_ok());
        assert!(executor
            .speculate_projector_by_domain("inventory", &events)
            .await
            .is_ok());
    }

    /// NotFound when no projector handles the domain.
    #[tokio::test]
    async fn test_projector_routing_not_found_for_unmatched_domain() {
        let mut projectors = HashMap::new();
        projectors.insert(
            "orders-projector".to_string(),
            (
                Arc::new(MockProjector) as Arc<dyn super::super::ProjectorHandler>,
                vec!["orders".to_string()],
            ),
        );

        let executor =
            SpeculativeExecutor::new(projectors, HashMap::new(), HashMap::new(), HashMap::new());

        let events = EventBook::default();
        let result = executor
            .speculate_projector_by_domain("inventory", &events)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    // ------------------------------------------------------------------------
    // Saga Domain Routing
    // ------------------------------------------------------------------------

    /// Saga is found when input_domain matches.
    #[tokio::test]
    async fn test_saga_routing_finds_by_input_domain() {
        let mut sagas = HashMap::new();
        sagas.insert(
            "order-fulfillment".to_string(),
            (
                Arc::new(MockSaga) as Arc<dyn super::super::SagaHandler>,
                "orders".to_string(),
            ),
        );

        let executor =
            SpeculativeExecutor::new(HashMap::new(), sagas, HashMap::new(), HashMap::new());

        let source = EventBook::default();
        let result = executor
            .speculate_saga_by_source_domain("orders", &source, &HashMap::new())
            .await;

        assert!(result.is_ok());
    }

    /// NotFound when no saga handles the source domain.
    #[tokio::test]
    async fn test_saga_routing_not_found_for_unmatched_domain() {
        let mut sagas = HashMap::new();
        sagas.insert(
            "order-fulfillment".to_string(),
            (
                Arc::new(MockSaga) as Arc<dyn super::super::SagaHandler>,
                "orders".to_string(),
            ),
        );

        let executor =
            SpeculativeExecutor::new(HashMap::new(), sagas, HashMap::new(), HashMap::new());

        let source = EventBook::default();
        let result = executor
            .speculate_saga_by_source_domain("inventory", &source, &HashMap::new())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    // ------------------------------------------------------------------------
    // Process Manager Domain Routing
    // ------------------------------------------------------------------------

    /// PM is found when trigger domain matches a subscription.
    #[tokio::test]
    async fn test_pm_routing_finds_by_subscription_domain() {
        let mut pms = HashMap::new();
        pms.insert(
            "order-flow".to_string(),
            (
                Arc::new(MockPM) as Arc<dyn ProcessManagerHandler>,
                "order-flow".to_string(),
                vec![Target::domain("orders"), Target::domain("inventory")],
            ),
        );

        let executor =
            SpeculativeExecutor::new(HashMap::new(), HashMap::new(), pms, HashMap::new());

        let trigger = EventBook::default();

        let result = executor
            .speculate_pm_by_trigger_domain("orders", &trigger, &HashMap::new())
            .await;
        assert!(result.is_ok());

        let result = executor
            .speculate_pm_by_trigger_domain("inventory", &trigger, &HashMap::new())
            .await;
        assert!(result.is_ok());
    }

    /// NotFound when no PM subscribes to the trigger domain.
    #[tokio::test]
    async fn test_pm_routing_not_found_for_unsubscribed_domain() {
        let mut pms = HashMap::new();
        pms.insert(
            "order-flow".to_string(),
            (
                Arc::new(MockPM) as Arc<dyn ProcessManagerHandler>,
                "order-flow".to_string(),
                vec![Target::domain("orders")],
            ),
        );

        let executor =
            SpeculativeExecutor::new(HashMap::new(), HashMap::new(), pms, HashMap::new());

        let trigger = EventBook::default();
        let result = executor
            .speculate_pm_by_trigger_domain("fulfillment", &trigger, &HashMap::new())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    /// Empty PM registry returns NotFound.
    #[tokio::test]
    async fn test_pm_routing_empty_pms_returns_not_found() {
        let executor = SpeculativeExecutor::new(
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );

        let trigger = EventBook::default();
        let result = executor
            .speculate_pm_by_trigger_domain("orders", &trigger, &HashMap::new())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }
}
