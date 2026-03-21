//! Tests for LocalAggregateContext and LocalAggregateContextFactory.
//!
//! The local aggregate context uses in-process SQLite storage with optional
//! service discovery for sync projectors. Key behaviors tested:
//! - Factory domain and client_logic accessors
//! - Context builder pattern (with_* methods)
//! - Helper functions: extract_sequence, build_event_book

use super::*;
use crate::bus::mock::MockEventBus;
use crate::discovery::StaticServiceDiscovery;
use crate::proto::{ContextualCommand, PageHeader};
use crate::standalone::DomainStorage;
use crate::storage::mock::{MockEventStore, MockSnapshotStore};

// ========================================================================
// Mock ClientLogic for testing
// ========================================================================

struct MockClientLogic;

impl MockClientLogic {
    fn new(_id: usize) -> Self {
        Self
    }
}

#[async_trait]
impl ClientLogic for MockClientLogic {
    async fn invoke(
        &self,
        _cmd: ContextualCommand,
    ) -> Result<crate::proto::BusinessResponse, Status> {
        use crate::proto::business_response::Result as BrResult;
        Ok(crate::proto::BusinessResponse {
            result: Some(BrResult::Events(EventBook::default())),
        })
    }
}

fn create_test_storage() -> DomainStorage {
    DomainStorage {
        event_store: Arc::new(MockEventStore::new()),
        snapshot_store: Arc::new(MockSnapshotStore::new()),
    }
}

fn create_test_factory(domain: &str, client_id: usize) -> LocalAggregateContextFactory {
    LocalAggregateContextFactory::new(
        domain.to_string(),
        create_test_storage(),
        Arc::new(StaticServiceDiscovery::new()),
        Arc::new(MockEventBus::new()),
        Arc::new(MockClientLogic::new(client_id)),
    )
}

// ========================================================================
// LocalAggregateContextFactory Tests
// ========================================================================

#[test]
fn test_factory_domain_returns_configured_domain() {
    let factory = create_test_factory("orders", 1);
    assert_eq!(factory.domain(), "orders");
}

#[test]
fn test_factory_domain_returns_different_domains() {
    let factory1 = create_test_factory("orders", 1);
    let factory2 = create_test_factory("inventory", 2);
    assert_eq!(factory1.domain(), "orders");
    assert_eq!(factory2.domain(), "inventory");
    assert_ne!(factory1.domain(), factory2.domain());
}

#[test]
fn test_factory_client_logic_returns_arc() {
    let factory = create_test_factory("orders", 42);
    let logic = factory.client_logic();
    // Verify we can clone the Arc (it's a shared reference)
    let _logic2 = logic.clone();
}

#[test]
fn test_factory_create_returns_context() {
    let factory = create_test_factory("orders", 1);
    let context = factory.create();
    // Verify context is created - we can't directly inspect it but we can ensure it's valid
    let _context2 = context;
}

#[test]
fn test_factory_with_dlq_publisher_returns_self() {
    let factory = create_test_factory("orders", 1);
    let updated = factory.with_dlq_publisher(Arc::new(NoopDeadLetterPublisher));
    // Verify domain is still correct
    assert_eq!(updated.domain(), "orders");
}

// ========================================================================
// LocalAggregateContext Builder Tests
// ========================================================================

#[test]
fn test_context_new_sets_defaults() {
    let storage = create_test_storage();
    let discovery = Arc::new(StaticServiceDiscovery::new());
    let bus = Arc::new(MockEventBus::new());

    let ctx = LocalAggregateContext::new(storage, discovery, bus);

    // Verify snapshot write is enabled by default
    assert!(ctx.snapshot_write_enabled);
}

#[test]
fn test_context_without_discovery() {
    let storage = create_test_storage();
    let bus = Arc::new(MockEventBus::new());

    let ctx = LocalAggregateContext::without_discovery(storage, bus);

    // Verify discovery is None
    assert!(ctx.discovery.is_none());
}

#[test]
fn test_context_with_snapshot_write_disabled() {
    let storage = create_test_storage();
    let discovery = Arc::new(StaticServiceDiscovery::new());
    let bus = Arc::new(MockEventBus::new());

    let ctx = LocalAggregateContext::new(storage, discovery, bus).with_snapshot_write_disabled();

    assert!(!ctx.snapshot_write_enabled);
}

#[test]
fn test_context_with_component_name() {
    let storage = create_test_storage();
    let discovery = Arc::new(StaticServiceDiscovery::new());
    let bus = Arc::new(MockEventBus::new());

    let ctx =
        LocalAggregateContext::new(storage, discovery, bus).with_component_name("my-aggregate");

    assert_eq!(ctx.component_name, "my-aggregate");
}

#[test]
fn test_context_with_sync_mode() {
    let storage = create_test_storage();
    let discovery = Arc::new(StaticServiceDiscovery::new());
    let bus = Arc::new(MockEventBus::new());

    let ctx = LocalAggregateContext::new(storage, discovery, bus)
        .with_sync_mode(crate::proto::SyncMode::Cascade);

    assert_eq!(ctx.sync_mode, Some(crate::proto::SyncMode::Cascade));
}

// ========================================================================
// Helper function tests
// ========================================================================

#[test]
fn test_extract_sequence_from_some() {
    let page = crate::proto::EventPage {
        header: Some(PageHeader {
            sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(5)),
        }),
        payload: None,
        created_at: None,
        committed: true,
        cascade_id: None,
    };
    assert_eq!(extract_sequence(Some(&page)), 5);
}

#[test]
fn test_extract_sequence_from_none() {
    assert_eq!(extract_sequence(None), 0);
}

#[test]
fn test_build_event_book_sets_cover() {
    let root = Uuid::new_v4();
    let book = build_event_book("orders", "angzarr", root, vec![], None);

    let cover = book.cover.as_ref().unwrap();
    assert_eq!(cover.domain, "orders");
    assert_eq!(cover.edition.as_ref().unwrap().name, "angzarr");
}

#[test]
fn test_build_event_book_with_pages() {
    let root = Uuid::new_v4();
    let pages = vec![
        crate::proto::EventPage {
            header: Some(PageHeader {
                sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(0)),
            }),
            payload: None,
            created_at: None,
            committed: true,
            cascade_id: None,
        },
        crate::proto::EventPage {
            header: Some(PageHeader {
                sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(1)),
            }),
            payload: None,
            created_at: None,
            committed: true,
            cascade_id: None,
        },
    ];
    let book = build_event_book("orders", "angzarr", root, pages, None);

    assert_eq!(book.pages.len(), 2);
}
