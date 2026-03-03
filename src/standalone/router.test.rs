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
use crate::proto::MergeStrategy;
use crate::proto_ext::CommandPageExt;

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

/// Commands created directly (not from saga) have no saga_origin.
#[test]
fn test_create_command_book_no_saga_origin() {
    let root = Uuid::new_v4();
    let command = create_command_book("orders", root, "CreateOrder", vec![]);

    assert!(command.saga_origin.is_none());
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
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};

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
            None,
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
            None,
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
            Some("test-edition".to_string()),
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
