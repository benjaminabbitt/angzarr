//! Tests for standalone command router.
//!
//! The command router dispatches commands to registered aggregate handlers:
//! - Routes commands by domain to appropriate client logic
//! - Manages per-domain storage isolation
//! - Supports sync projectors and sagas for CASCADE mode
//! - Provides speculative execution for "what-if" scenarios
//!
//! Why this matters: The router is the central dispatch point for all commands
//! in standalone mode. If routing fails, commands go to wrong handlers or fail
//! entirely. If storage lookup fails, events are lost or written to wrong stores.
//!
//! Key behaviors verified:
//! - Test helper creates valid CommandBook structures
//! - Router tracks registered domains correctly
//! - Storage lookup returns correct per-domain stores
//! - DomainStorage and SyncProjectorEntry are Clone/Send

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use uuid::Uuid;

use super::*;
use crate::proto::{Cover, MergeStrategy, Uuid as ProtoUuid};
use crate::proto_ext::CommandPageExt;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create a command book for tests.
pub fn create_command_book(
    domain: &str,
    root: Uuid,
    command_type: &str,
    command_data: Vec<u8>,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![crate::proto::CommandPage {
            header: Some(crate::proto::PageHeader {
                sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(0)),
            }),
            payload: Some(crate::proto::command_page::Payload::Command(
                prost_types::Any {
                    type_url: command_type.to_string(),
                    value: command_data,
                },
            )),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
    }
}

// ============================================================================
// Helper Construction Tests
// ============================================================================

/// create_command_book helper produces valid structure.
///
/// Validates the test helper itself so other tests can rely on it.
#[test]
fn test_create_command_book_basic() {
    let root = Uuid::new_v4();
    let command = create_command_book("orders", root, "CreateOrder", vec![1, 2, 3]);

    let cover = command.cover.as_ref().unwrap();
    assert_eq!(cover.domain, "orders");
    assert!(cover.correlation_id.is_empty());
    assert!(cover.edition.is_none());
}

/// Pages contain command payload with correct type and data.
#[test]
fn test_create_command_book_pages() {
    let root = Uuid::new_v4();
    let command = create_command_book("orders", root, "CreateOrder", vec![1, 2, 3]);

    assert_eq!(command.pages.len(), 1);
    let page = &command.pages[0];
    assert_eq!(page.sequence_num(), 0);
    assert_eq!(page.merge_strategy, MergeStrategy::MergeCommutative as i32);

    if let Some(crate::proto::command_page::Payload::Command(ref cmd)) = page.payload {
        assert_eq!(cmd.type_url, "CreateOrder");
        assert_eq!(cmd.value, vec![1, 2, 3]);
    } else {
        panic!("Expected Command payload");
    }
}

/// Root UUID is correctly encoded in cover.
#[test]
fn test_create_command_book_root_uuid() {
    let root = Uuid::new_v4();
    let command = create_command_book("orders", root, "CreateOrder", vec![]);

    let cover = command.cover.as_ref().unwrap();
    let proto_uuid = cover.root.as_ref().unwrap();
    let extracted_root = Uuid::from_slice(&proto_uuid.value).unwrap();
    assert_eq!(extracted_root, root);
}

/// Empty command data is valid (some commands have no payload).
#[test]
fn test_create_command_book_empty_data() {
    let root = Uuid::new_v4();
    let command = create_command_book("test", root, "EmptyCommand", vec![]);

    assert!(!command.pages.is_empty());
    if let Some(crate::proto::command_page::Payload::Command(ref cmd)) = command.pages[0].payload {
        assert!(cmd.value.is_empty());
    }
}

/// Commands created directly have no angzarr_deferred provenance (they use explicit sequence).
#[test]
fn test_create_command_book_explicit_sequence() {
    use crate::proto_ext::CommandPageExt;
    let root = Uuid::new_v4();
    let command = create_command_book("orders", root, "CreateOrder", vec![]);

    // Direct commands get explicit sequence 0, not deferred
    assert_eq!(command.pages[0].sequence_num(), 0);
    assert!(!command.pages[0].is_deferred());
}

// ============================================================================
// DomainStorage Tests
// ============================================================================

/// DomainStorage must be Clone for router construction.
#[test]
fn test_domain_storage_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<DomainStorage>();
}

/// event_book_repo creates repository from storage components.
///
/// The helper consolidates the repeated pattern of creating EventBookRepository
/// from event_store and snapshot_store. Verifies the method doesn't panic.
#[test]
fn test_domain_storage_event_book_repo() {
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};

    let storage = DomainStorage {
        event_store: Arc::new(MockEventStore::new()),
        snapshot_store: Arc::new(MockSnapshotStore::new()),
    };

    // Should create repository without panicking
    let _repo = storage.event_book_repo();
}

// ============================================================================
// CommandRouter Construction Tests
// ============================================================================

mod router_construction {
    use super::*;
    use crate::bus::MockEventBus;
    use crate::discovery::StaticServiceDiscovery;
    use crate::orchestration::aggregate::ClientLogic;
    use crate::proto::{BusinessResponse, ContextualCommand};
    use crate::storage::mock::{MockEventStore, MockPositionStore, MockSnapshotStore};

    fn make_router_empty() -> CommandRouter {
        let business = HashMap::new();
        let stores = HashMap::new();
        let discovery = Arc::new(StaticServiceDiscovery::new());
        let event_bus = Arc::new(MockEventBus::new());
        let sync_projectors = vec![];

        CommandRouter::new(
            business,
            stores,
            discovery,
            event_bus,
            sync_projectors,
            vec![],
            vec![],
            None,
            Arc::new(MockPositionStore::new()),
        )
    }

    fn make_router_with_domains(domains: &[&str]) -> CommandRouter {
        struct DummyLogic;

        #[async_trait]
        impl ClientLogic for DummyLogic {
            async fn invoke(&self, _cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
                Ok(BusinessResponse::default())
            }
        }

        let mut business: HashMap<String, Arc<dyn ClientLogic>> = HashMap::new();
        let mut stores: HashMap<String, DomainStorage> = HashMap::new();

        for domain in domains {
            business.insert(domain.to_string(), Arc::new(DummyLogic));
            stores.insert(
                domain.to_string(),
                DomainStorage {
                    event_store: Arc::new(MockEventStore::new()),
                    snapshot_store: Arc::new(MockSnapshotStore::new()),
                },
            );
        }

        let discovery = Arc::new(StaticServiceDiscovery::new());
        let event_bus = Arc::new(MockEventBus::new());
        let sync_projectors = vec![];

        CommandRouter::new(
            business,
            stores,
            discovery,
            event_bus,
            sync_projectors,
            vec![],
            vec![],
            None,
            Arc::new(MockPositionStore::new()),
        )
    }

    /// Empty router has no registered domains.
    #[test]
    fn test_router_empty_construction() {
        let router = make_router_empty();
        assert!(router.domains().is_empty());
    }

    /// domains() returns all registered domain names.
    #[test]
    fn test_router_domains_returned() {
        let router = make_router_with_domains(&["orders", "inventory", "fulfillment"]);
        let domains = router.domains();

        assert_eq!(domains.len(), 3);
        assert!(domains.contains(&"orders"));
        assert!(domains.contains(&"inventory"));
        assert!(domains.contains(&"fulfillment"));
    }

    /// has_handler returns true for registered domains.
    #[test]
    fn test_router_has_handler_true() {
        let router = make_router_with_domains(&["orders", "inventory"]);

        assert!(router.has_handler("orders"));
        assert!(router.has_handler("inventory"));
    }

    /// has_handler returns false for unregistered domains.
    #[test]
    fn test_router_has_handler_false() {
        let router = make_router_with_domains(&["orders"]);

        assert!(!router.has_handler("unknown"));
        assert!(!router.has_handler("inventory"));
    }

    /// get_storage succeeds for registered domains.
    #[test]
    fn test_router_get_storage_success() {
        let router = make_router_with_domains(&["orders"]);

        let result = router.get_storage("orders");
        assert!(result.is_ok());
    }

    /// get_storage returns NotFound for unregistered domains.
    #[test]
    fn test_router_get_storage_not_found() {
        let router = make_router_with_domains(&["orders"]);

        let result = router.get_storage("unknown");
        assert!(result.is_err());
    }

    /// get_storage error message includes the missing domain name.
    #[test]
    fn test_router_get_storage_error_message() {
        let router = make_router_with_domains(&["orders"]);

        let result = router.get_storage("missing_domain");
        match result {
            Err(err) => {
                assert!(err.message().contains("missing_domain"));
                assert!(err.message().contains("No storage configured"));
            }
            Ok(_) => panic!("Expected error"),
        }
    }

    // ============================================================================
    // get_domain_resources Tests
    // ============================================================================

    /// get_domain_resources succeeds for registered domains.
    ///
    /// Returns both business handler and storage for the domain.
    /// This helper consolidates the repeated pattern of fetching both resources.
    #[test]
    fn test_get_domain_resources_success() {
        let router = make_router_with_domains(&["orders"]);

        let result = router.get_domain_resources("orders");
        assert!(result.is_ok());
    }

    /// get_domain_resources returns NotFound when handler is missing.
    ///
    /// If a domain has storage but no handler registered, returns handler error.
    #[test]
    fn test_get_domain_resources_missing_handler() {
        let router = make_router_empty();

        let result = router.get_domain_resources("orders");
        match result {
            Err(err) => {
                assert!(err.message().contains("No handler registered"));
                assert!(err.message().contains("orders"));
            }
            Ok(_) => panic!("Expected error"),
        }
    }

    /// get_domain_resources error includes domain name for debugging.
    #[test]
    fn test_get_domain_resources_error_includes_domain() {
        let router = make_router_with_domains(&["orders"]);

        let result = router.get_domain_resources("missing_domain");
        match result {
            Err(err) => {
                assert!(err.message().contains("missing_domain"));
            }
            Ok(_) => panic!("Expected error"),
        }
    }

    /// Router can be constructed with edition name for edition isolation.
    #[test]
    fn test_router_with_edition() {
        let business = HashMap::new();
        let stores = HashMap::new();
        let discovery = Arc::new(StaticServiceDiscovery::new());
        let event_bus = Arc::new(MockEventBus::new());
        let sync_projectors = vec![];

        let router = CommandRouter::new(
            business,
            stores,
            discovery,
            event_bus,
            sync_projectors,
            vec![],
            vec![],
            Some("test-edition".to_string()),
            Arc::new(MockPositionStore::new()),
        );

        assert!(router.domains().is_empty());
    }
}

// ============================================================================
// SyncProjectorEntry Tests
// ============================================================================

mod sync_projector_tests {
    use super::*;

    /// SyncProjectorEntry must be Send for async execution.
    #[test]
    fn test_sync_projector_entry_name() {
        fn assert_sync<T: Send>() {}
        assert_sync::<SyncProjectorEntry>();
    }
}

// ============================================================================
// SyncPMEntry Tests
// ============================================================================

mod sync_pm_tests {
    use super::*;
    use crate::bus::MockEventBus;
    use crate::descriptor::Target;
    use crate::discovery::StaticServiceDiscovery;
    use crate::orchestration::aggregate::ClientLogic;
    use crate::proto::{BusinessResponse, ContextualCommand, Edition, EventBook, EventPage};
    use crate::proto_ext::CoverExt;
    use crate::standalone::traits::{ProcessManagerHandleResult, ProcessManagerHandler};
    use crate::storage::mock::{MockEventStore, MockPositionStore, MockSnapshotStore};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// SyncPMEntry must be Send for async execution.
    ///
    /// Why: Sync PMs are called from async context in execute_with_cascade().
    /// Must be Send to cross await boundaries.
    #[test]
    fn test_sync_pm_entry_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SyncPMEntry>();
    }

    /// SyncPMEntry must be Clone for router construction.
    ///
    /// Why: Router stores Vec<SyncPMEntry>, needs to clone for iteration.
    #[test]
    fn test_sync_pm_entry_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<SyncPMEntry>();
    }

    // ========================================================================
    // Mock PM Handler
    // ========================================================================

    /// Mock PM handler that tracks whether methods were called.
    #[derive(Default)]
    struct MockPMHandler {
        prepare_called: AtomicBool,
        handle_called: AtomicBool,
    }

    impl MockPMHandler {
        fn new() -> Self {
            Self::default()
        }

        fn was_prepare_called(&self) -> bool {
            self.prepare_called.load(Ordering::SeqCst)
        }

        fn was_handle_called(&self) -> bool {
            self.handle_called.load(Ordering::SeqCst)
        }
    }

    impl ProcessManagerHandler for MockPMHandler {
        fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
            self.prepare_called.store(true, Ordering::SeqCst);
            vec![] // No additional destinations
        }

        fn handle(
            &self,
            _trigger: &EventBook,
            _process_state: Option<&EventBook>,
            _destinations: &[EventBook],
        ) -> ProcessManagerHandleResult {
            self.handle_called.store(true, Ordering::SeqCst);
            ProcessManagerHandleResult {
                commands: vec![],
                process_events: None,
                facts: vec![],
            }
        }
    }

    // ========================================================================
    // Test Helpers
    // ========================================================================

    fn make_router_with_pm(pm_entry: SyncPMEntry, domains: &[&str]) -> CommandRouter {
        struct DummyLogic;

        #[async_trait]
        impl ClientLogic for DummyLogic {
            async fn invoke(&self, _cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
                Ok(BusinessResponse::default())
            }
        }

        let mut business: HashMap<String, Arc<dyn ClientLogic>> = HashMap::new();
        let mut stores: HashMap<String, DomainStorage> = HashMap::new();

        for domain in domains {
            business.insert(domain.to_string(), Arc::new(DummyLogic));
            stores.insert(
                domain.to_string(),
                DomainStorage {
                    event_store: Arc::new(MockEventStore::new()),
                    snapshot_store: Arc::new(MockSnapshotStore::new()),
                },
            );
        }

        let discovery = Arc::new(StaticServiceDiscovery::new());
        let event_bus = Arc::new(MockEventBus::new());

        CommandRouter::new(
            business,
            stores,
            discovery,
            event_bus,
            vec![], // sync_projectors
            vec![], // sync_sagas
            vec![pm_entry],
            None,
            Arc::new(MockPositionStore::new()),
        )
    }

    fn make_event_book(domain: &str, root: Uuid, correlation_id: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(crate::proto::Uuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: Some(Edition {
                    name: String::new(),
                    divergences: vec![],
                }),
            }),
            pages: vec![EventPage {
                header: Some(crate::proto::PageHeader {
                    sequence_type: Some(crate::proto::page_header::SequenceType::Sequence(0)),
                }),
                created_at: None,
                payload: None,
            }],
            snapshot: None,
            next_sequence: 1,
        }
    }

    // ========================================================================
    // Correlation ID Requirement Tests
    // ========================================================================

    /// PMs are skipped when EventBook has no correlation_id.
    ///
    /// Why: PMs correlate events across domains via correlation_id. Without it,
    /// the PM cannot track the cross-domain workflow. Events without correlation_id
    /// are valid (single-domain flows) but PMs should not process them.
    #[tokio::test]
    async fn test_pm_skipped_without_correlation_id() {
        let handler = Arc::new(MockPMHandler::new());
        let pm_entry = SyncPMEntry {
            name: "test-pm".to_string(),
            handler: handler.clone(),
            pm_domain: "pm-state".to_string(),
            subscriptions: vec![Target {
                domain: "orders".to_string(),
                types: vec![],
            }],
        };

        let router = make_router_with_pm(pm_entry, &["orders", "pm-state"]);

        // Event with empty correlation_id
        let events = make_event_book("orders", Uuid::new_v4(), "");

        // call_sync_pms should skip PM due to missing correlation_id
        router.call_sync_pms(&events).await.unwrap();

        assert!(
            !handler.was_prepare_called(),
            "PM prepare() should not be called without correlation_id"
        );
        assert!(
            !handler.was_handle_called(),
            "PM handle() should not be called without correlation_id"
        );
    }

    // ========================================================================
    // Subscription Matching Tests
    // ========================================================================

    /// PMs are only called when subscription domain matches trigger domain.
    ///
    /// Why: A PM subscribing to "orders" should not be called when "inventory"
    /// events arrive. Subscription filtering ensures PMs only see relevant events.
    #[tokio::test]
    async fn test_pm_skipped_for_non_matching_subscription() {
        let handler = Arc::new(MockPMHandler::new());
        let pm_entry = SyncPMEntry {
            name: "test-pm".to_string(),
            handler: handler.clone(),
            pm_domain: "pm-state".to_string(),
            subscriptions: vec![Target {
                domain: "orders".to_string(), // Subscribed to orders
                types: vec![],
            }],
        };

        let router = make_router_with_pm(pm_entry, &["orders", "inventory", "pm-state"]);

        // Event from inventory domain (PM not subscribed)
        let events = make_event_book("inventory", Uuid::new_v4(), "corr-123");

        router.call_sync_pms(&events).await.unwrap();

        assert!(
            !handler.was_prepare_called(),
            "PM should not be called for non-matching domain"
        );
    }

    /// PMs are called when subscription domain matches trigger domain.
    ///
    /// Why: Basic happy path - PM subscribed to "orders" receives order events.
    #[tokio::test]
    async fn test_pm_called_for_matching_subscription() {
        let handler = Arc::new(MockPMHandler::new());
        let pm_entry = SyncPMEntry {
            name: "test-pm".to_string(),
            handler: handler.clone(),
            pm_domain: "pm-state".to_string(),
            subscriptions: vec![Target {
                domain: "orders".to_string(),
                types: vec![],
            }],
        };

        let router = make_router_with_pm(pm_entry, &["orders", "pm-state"]);

        // Event from orders domain (PM subscribed)
        let events = make_event_book("orders", Uuid::new_v4(), "corr-123");

        router.call_sync_pms(&events).await.unwrap();

        assert!(
            handler.was_prepare_called(),
            "PM prepare() should be called for matching domain"
        );
        assert!(
            handler.was_handle_called(),
            "PM handle() should be called for matching domain"
        );
    }

    // ========================================================================
    // Infrastructure Domain Skipping Tests
    // ========================================================================

    /// PMs skip events from infrastructure domains (prefixed with _).
    ///
    /// Why: Infrastructure domains like _topology or _metrics contain
    /// system-level events that shouldn't trigger business PMs.
    #[tokio::test]
    async fn test_pm_skipped_for_infrastructure_domain() {
        let handler = Arc::new(MockPMHandler::new());
        let pm_entry = SyncPMEntry {
            name: "test-pm".to_string(),
            handler: handler.clone(),
            pm_domain: "pm-state".to_string(),
            subscriptions: vec![Target {
                domain: "_infrastructure".to_string(),
                types: vec![],
            }],
        };

        let router = make_router_with_pm(pm_entry, &["_infrastructure", "pm-state"]);

        let events = make_event_book("_infrastructure", Uuid::new_v4(), "corr-123");

        router.call_sync_pms(&events).await.unwrap();

        assert!(
            !handler.was_prepare_called(),
            "PM should skip infrastructure domains"
        );
    }
}
