//! Entity service (BusinessCoordinator).

use std::sync::Arc;

use tonic::{Request, Response, Status};
use tracing::warn;

use crate::interfaces::event_store::StorageError;
use crate::interfaces::{BusinessError, BusinessLogicClient, EventBus, EventStore, SnapshotStore};
use crate::proto::{
    business_coordinator_server::BusinessCoordinator, business_response, CommandBook,
    CommandResponse, EventBook,
};
use crate::repository::EventBookRepository;

/// Maximum number of retries for auto_resequence on sequence conflicts.
const MAX_RESEQUENCE_RETRIES: u32 = 3;

/// Entity service.
///
/// Receives commands, loads prior state, calls business logic,
/// persists new events, and notifies projectors/sagas.
pub struct EntityService {
    event_store: Arc<dyn EventStore>,
    event_book_repo: Arc<EventBookRepository>,
    snapshot_store: Arc<dyn SnapshotStore>,
    business_client: Arc<dyn BusinessLogicClient>,
    event_bus: Arc<dyn EventBus>,
    /// When false, snapshots are not written even if business logic returns snapshot_state.
    snapshot_write_enabled: bool,
}

impl EntityService {
    /// Create a new entity service with snapshots enabled.
    pub fn new(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: Arc<dyn BusinessLogicClient>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            event_store: Arc::clone(&event_store),
            event_book_repo: Arc::new(EventBookRepository::new(
                event_store,
                Arc::clone(&snapshot_store),
            )),
            snapshot_store,
            business_client,
            event_bus,
            snapshot_write_enabled: true,
        }
    }

    /// Create a new entity service with configurable snapshot behavior.
    pub fn with_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: Arc<dyn BusinessLogicClient>,
        event_bus: Arc<dyn EventBus>,
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> Self {
        Self {
            event_store: Arc::clone(&event_store),
            event_book_repo: Arc::new(EventBookRepository::with_config(
                event_store,
                Arc::clone(&snapshot_store),
                snapshot_read_enabled,
            )),
            snapshot_store,
            business_client,
            event_bus,
            snapshot_write_enabled,
        }
    }
}

#[tonic::async_trait]
impl BusinessCoordinator for EntityService {
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command_book = request.into_inner();
        let auto_resequence = command_book.auto_resequence;

        // Extract cover (aggregate identity)
        let cover = command_book
            .cover
            .clone()
            .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

        let domain = cover.domain.clone();
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        // Validate domain is supported
        if !self.business_client.has_domain(&domain) {
            return Err(Status::not_found(format!(
                "Domain '{}' not registered. Available: {:?}",
                domain,
                self.business_client.domains()
            )));
        }

        // Generate correlation ID if not provided using UUIDv5
        // Uses command body hash for deterministic but unique IDs
        let correlation_id = if command_book.correlation_id.is_empty() {
            use prost::Message;
            let mut buf = Vec::new();
            command_book.encode(&mut buf).map_err(|e| {
                Status::internal(format!("Failed to encode command for correlation ID: {e}"))
            })?;
            // Create angzarr namespace from DNS namespace
            let angzarr_ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev");
            uuid::Uuid::new_v5(&angzarr_ns, &buf).to_string()
        } else {
            command_book.correlation_id.clone()
        };

        // Retry loop for auto_resequence
        let mut attempt = 0;
        loop {
            attempt += 1;

            // Validate CommandBook has pages
            let first_page = command_book.pages.first().ok_or_else(|| {
                Status::invalid_argument("CommandBook must have at least one page")
            })?;

            // 1. Quick sequence check (avoids loading full events if stale)
            // Skip pre-validation when auto_resequence is enabled - rely on write-time validation
            if !auto_resequence {
                let expected_sequence = first_page.sequence;

                // Query current aggregate sequence (lightweight operation)
                let next_sequence = self
                    .event_store
                    .get_next_sequence(&domain, root_uuid)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

                // Validate sequence before loading full events
                if expected_sequence != next_sequence {
                    return Err(Status::failed_precondition(format!(
                        "Sequence mismatch: command expects {}, aggregate at {}",
                        expected_sequence, next_sequence
                    )));
                }
            }

            // 2. Load prior state (only after sequence validation passes)
            let prior_events = self
                .event_book_repo
                .get(&domain, root_uuid)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;


            // Create contextual command
            let contextual_command = crate::proto::ContextualCommand {
                events: Some(prior_events),
                command: Some(command_book.clone()),
            };

            // Call business logic
            let response = self
                .business_client
                .handle(&domain, contextual_command)
                .await
                .map_err(|e| match e {
                    BusinessError::DomainNotFound(d) => {
                        Status::not_found(format!("Domain not found: {d}"))
                    }
                    BusinessError::Rejected(msg) => Status::failed_precondition(msg),
                    BusinessError::Timeout { domain } => {
                        Status::deadline_exceeded(format!("Timeout waiting for domain: {domain}"))
                    }
                    BusinessError::Connection { domain, message } => {
                        Status::unavailable(format!("Connection to {domain} failed: {message}"))
                    }
                    BusinessError::Grpc(status) => *status,
                })?;

            // Extract events from BusinessResponse
            let mut new_events = match response.result {
                Some(business_response::Result::Events(events)) => events,
                Some(business_response::Result::Revocation(revocation)) => {
                    // Business logic explicitly requested framework handling
                    // TODO: Implement proper revocation handling in saga_compensation module
                    return Err(Status::failed_precondition(format!(
                        "Command revoked: {}",
                        revocation.reason
                    )));
                }
                None => {
                    // Empty response - return empty EventBook
                    EventBook {
                        cover: None,
                        pages: vec![],
                        snapshot: None,
                        correlation_id: String::new(),
                        snapshot_state: None,
                    }
                }
            };

            // Propagate correlation ID from command to events
            new_events.correlation_id = correlation_id.clone();

            // Persist new events - may fail with sequence conflict
            match self.event_book_repo.put(&new_events).await {
                Ok(()) => {
                    // Success - store snapshot and publish
                    // Store snapshot if business logic provided state and writing is enabled
                    // Snapshot sequence is the sequence of the last event (business logic provides explicit sequences)
                    if self.snapshot_write_enabled {
                        if let Some(ref state) = new_events.snapshot_state {
                            // Get sequence from last event page - business logic must provide explicit Sequence::Num
                            let snapshot_sequence = new_events
                                .pages
                                .last()
                                .and_then(|p| match &p.sequence {
                                    Some(crate::proto::event_page::Sequence::Num(n)) => Some(*n + 1),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            let snapshot = crate::proto::Snapshot {
                                sequence: snapshot_sequence,
                                state: Some(state.clone()),
                            };
                            self.snapshot_store
                                .put(&domain, root_uuid, snapshot)
                                .await
                                .map_err(|e| {
                                    Status::internal(format!("Failed to persist snapshot: {e}"))
                                })?;
                        }
                    }

                    // Wrap in Arc for immutable distribution
                    let new_events = Arc::new(new_events);

                    // Notify event bus (projectors, sagas)
                    let publish_result = self
                        .event_bus
                        .publish(Arc::clone(&new_events))
                        .await
                        .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

                    return Ok(Response::new(CommandResponse {
                        events: Some(
                            Arc::try_unwrap(new_events).unwrap_or_else(|arc| (*arc).clone()),
                        ),
                        projections: publish_result.projections,
                    }));
                }
                Err(StorageError::SequenceConflict { expected, actual }) => {
                    // Sequence conflict - retry if auto_resequence is enabled
                    if auto_resequence && attempt < MAX_RESEQUENCE_RETRIES {
                        warn!(
                            domain = %domain,
                            root = %root_uuid,
                            attempt = attempt,
                            expected = expected,
                            actual = actual,
                            "Sequence conflict, retrying with fresh state"
                        );
                        continue;
                    } else if auto_resequence {
                        return Err(Status::aborted(format!(
                            "Sequence conflict after {} retries: expected {}, got {}",
                            MAX_RESEQUENCE_RETRIES, expected, actual
                        )));
                    } else {
                        return Err(Status::aborted(format!(
                            "Sequence conflict: expected {}, got {} (auto_resequence disabled)",
                            expected, actual
                        )));
                    }
                }
                Err(e) => {
                    return Err(Status::internal(format!("Failed to persist events: {e}")));
                }
            }
        }
    }

    async fn record(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let event_book = request.into_inner();

        // Persist events directly (used by sagas)
        self.event_book_repo
            .put(&event_book)
            .await
            .map_err(|e| Status::internal(format!("Failed to persist events: {e}")))?;

        // Store snapshot if business logic provided state and writing is enabled
        // Compute sequence from the last event in the book
        if self.snapshot_write_enabled {
            if let Some(ref state) = event_book.snapshot_state {
                if let Some(ref cover) = event_book.cover {
                    if let Some(ref root) = cover.root {
                        let root_uuid = uuid::Uuid::from_slice(&root.value)
                            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

                        // Compute sequence from last event page
                        let snapshot_sequence = event_book
                            .pages
                            .last()
                            .and_then(|p| match &p.sequence {
                                Some(crate::proto::event_page::Sequence::Num(n)) => Some(*n + 1),
                                _ => None,
                            })
                            .unwrap_or(0);

                        let snapshot = crate::proto::Snapshot {
                            sequence: snapshot_sequence,
                            state: Some(state.clone()),
                        };
                        self.snapshot_store
                            .put(&cover.domain, root_uuid, snapshot)
                            .await
                            .map_err(|e| {
                                Status::internal(format!("Failed to persist snapshot: {e}"))
                            })?;
                    }
                }
            }
        }

        // Wrap in Arc for immutable distribution
        let event_book = Arc::new(event_book);

        // Notify event bus
        let publish_result = self
            .event_bus
            .publish(Arc::clone(&event_book))
            .await
            .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

        Ok(Response::new(CommandResponse {
            events: Some(Arc::try_unwrap(event_book).unwrap_or_else(|arc| (*arc).clone())),
            projections: publish_result.projections,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandPage, Cover, Uuid as ProtoUuid};
    use crate::test_utils::{MockBusinessLogic, MockEventBus, MockEventStore, MockSnapshotStore};
    use prost_types::Any;

    fn create_test_service_with_mocks(
        event_store: Arc<MockEventStore>,
        snapshot_store: Arc<MockSnapshotStore>,
        business_client: Arc<MockBusinessLogic>,
        event_bus: Arc<MockEventBus>,
    ) -> EntityService {
        EntityService::new(event_store, snapshot_store, business_client, event_bus)
    }

    fn create_default_test_service() -> (
        EntityService,
        Arc<MockEventStore>,
        Arc<MockSnapshotStore>,
        Arc<MockBusinessLogic>,
        Arc<MockEventBus>,
    ) {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let business_client = Arc::new(MockBusinessLogic::new(vec!["orders".to_string()]));
        let event_bus = Arc::new(MockEventBus::new());

        let service = create_test_service_with_mocks(
            event_store.clone(),
            snapshot_store.clone(),
            business_client.clone(),
            event_bus.clone(),
        );

        (
            service,
            event_store,
            snapshot_store,
            business_client,
            event_bus,
        )
    }

    fn make_command_book(domain: &str, root_bytes: Vec<u8>) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid { value: root_bytes }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(Any {
                    type_url: "test.CreateOrder".to_string(),
                    value: vec![],
                }),
                synchronous: false,
            }],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        }
    }

    #[tokio::test]
    async fn test_handle_command_success() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.events.is_some());
        assert_eq!(event_bus.published_count().await, 1);
    }

    #[tokio::test]
    async fn test_handle_command_missing_cover() {
        let (service, _, _, _, _) = create_default_test_service();
        let command = CommandBook {
            cover: None,
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        };

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("cover"));
    }

    #[tokio::test]
    async fn test_handle_command_missing_root() {
        let (service, _, _, _, _) = create_default_test_service();
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
            }),
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        };

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("root"));
    }

    #[tokio::test]
    async fn test_handle_command_invalid_uuid() {
        let (service, _, _, _, _) = create_default_test_service();
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3], // Invalid: must be 16 bytes
                }),
            }),
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        };

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("UUID"));
    }

    #[tokio::test]
    async fn test_handle_command_unknown_domain() {
        let (service, _, _, _, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();
        let command = make_command_book("unknown_domain", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_rejects() {
        let (service, _, _, business_client, _) = create_default_test_service();
        business_client.set_reject_command(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_connection_failure() {
        let (service, _, _, business_client, _) = create_default_test_service();
        business_client.set_fail_on_handle(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn test_handle_command_event_bus_failure() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        event_bus.set_fail_on_publish(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[tokio::test]
    async fn test_record_events_success() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let event_book = crate::proto::EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![crate::proto::EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.OrderCreated".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let response = service.record(Request::new(event_book)).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.events.is_some());
        assert_eq!(event_bus.published_count().await, 1);
    }

    #[tokio::test]
    async fn test_record_events_bus_failure() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        event_bus.set_fail_on_publish(true).await;

        let root = uuid::Uuid::new_v4();
        let event_book = crate::proto::EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let response = service.record(Request::new(event_book)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[tokio::test]
    async fn test_handle_command_stores_snapshot_when_returned() {
        let (service, _, snapshot_store, business_client, _) = create_default_test_service();
        business_client.set_return_snapshot(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;
        assert!(response.is_ok());

        // Verify snapshot was stored
        let stored = snapshot_store.get_stored("orders", root).await;
        assert!(stored.is_some());
        let snapshot = stored.unwrap();
        assert_eq!(snapshot.sequence, 1); // First event, so snapshot at seq 1
    }

    #[tokio::test]
    async fn test_handle_command_no_snapshot_when_not_returned() {
        let (service, _, snapshot_store, _, _) = create_default_test_service();
        // Default is no snapshot

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;
        assert!(response.is_ok());

        // Verify no snapshot was stored
        let stored = snapshot_store.get_stored("orders", root).await;
        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn test_record_events_stores_snapshot() {
        let (service, _, snapshot_store, _, _) = create_default_test_service();

        let root = uuid::Uuid::new_v4();
        let event_book = crate::proto::EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![crate::proto::EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.OrderCreated".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None, // Framework-populated on load, not set by business logic
            correlation_id: String::new(),
            // Business logic sets snapshot_state; framework computes sequence from events
            snapshot_state: Some(Any {
                type_url: "test.OrderState".to_string(),
                value: vec![4, 5, 6],
            }),
        };

        let response = service.record(Request::new(event_book)).await;
        assert!(response.is_ok());

        // Verify snapshot was stored with sequence computed from last event (0 + 1 = 1)
        let stored = snapshot_store.get_stored("orders", root).await;
        assert!(stored.is_some());
        let snapshot = stored.unwrap();
        assert_eq!(snapshot.sequence, 1);
    }

    fn create_test_service_with_snapshot_config(
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> (
        EntityService,
        Arc<MockEventStore>,
        Arc<MockSnapshotStore>,
        Arc<MockBusinessLogic>,
        Arc<MockEventBus>,
    ) {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let business_client = Arc::new(MockBusinessLogic::new(vec!["orders".to_string()]));
        let event_bus = Arc::new(MockEventBus::new());

        let service = EntityService::with_config(
            event_store.clone(),
            snapshot_store.clone(),
            business_client.clone(),
            event_bus.clone(),
            snapshot_read_enabled,
            snapshot_write_enabled,
        );

        (
            service,
            event_store,
            snapshot_store,
            business_client,
            event_bus,
        )
    }

    #[tokio::test]
    async fn test_handle_command_no_snapshot_stored_when_write_disabled() {
        let (service, _, snapshot_store, business_client, _) =
            create_test_service_with_snapshot_config(true, false);
        business_client.set_return_snapshot(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;
        assert!(response.is_ok());

        // Verify no snapshot was stored even though business logic returned one
        let stored = snapshot_store.get_stored("orders", root).await;
        assert!(
            stored.is_none(),
            "Snapshot should not be stored when write is disabled"
        );
    }

    #[tokio::test]
    async fn test_record_events_no_snapshot_stored_when_write_disabled() {
        let (service, _, snapshot_store, _, _) =
            create_test_service_with_snapshot_config(true, false);

        let root = uuid::Uuid::new_v4();
        let event_book = crate::proto::EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![crate::proto::EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.OrderCreated".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: Some(Any {
                type_url: "test.OrderState".to_string(),
                value: vec![4, 5, 6],
            }),
        };

        let response = service.record(Request::new(event_book)).await;
        assert!(response.is_ok());

        // Verify no snapshot was stored
        let stored = snapshot_store.get_stored("orders", root).await;
        assert!(
            stored.is_none(),
            "Snapshot should not be stored when write is disabled"
        );
    }

    #[tokio::test]
    async fn test_handle_command_snapshot_stored_when_write_enabled() {
        let (service, _, snapshot_store, business_client, _) =
            create_test_service_with_snapshot_config(true, true);
        business_client.set_return_snapshot(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;
        assert!(response.is_ok());

        // Verify snapshot was stored
        let stored = snapshot_store.get_stored("orders", root).await;
        assert!(
            stored.is_some(),
            "Snapshot should be stored when write is enabled"
        );
        assert_eq!(stored.unwrap().sequence, 1);
    }

    // ========== Sequence Validation Tests ==========

    fn make_command_book_with_sequence(
        domain: &str,
        root_bytes: Vec<u8>,
        sequence: u32,
        auto_resequence: bool,
    ) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid { value: root_bytes }),
            }),
            pages: vec![CommandPage {
                sequence,
                command: Some(Any {
                    type_url: "test.CreateOrder".to_string(),
                    value: vec![],
                }),
                synchronous: false,
            }],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence,
            fact: false,
        }
    }

    #[tokio::test]
    async fn test_handle_command_sequence_match_succeeds() {
        let (service, event_store, _, _, _) = create_default_test_service();

        // Set aggregate at sequence 5
        event_store.set_next_sequence(5).await;

        let root = uuid::Uuid::new_v4();
        // Command expects sequence 5 (matches aggregate state)
        let command = make_command_book_with_sequence("orders", root.as_bytes().to_vec(), 5, false);

        let response = service.handle(Request::new(command)).await;
        assert!(
            response.is_ok(),
            "Command with matching sequence should succeed"
        );
    }

    #[tokio::test]
    async fn test_handle_command_sequence_mismatch_fails_without_auto_resequence() {
        let (service, event_store, _, _, _) = create_default_test_service();

        // Set aggregate at sequence 5
        event_store.set_next_sequence(5).await;

        let root = uuid::Uuid::new_v4();
        // Command expects sequence 0, but aggregate is at 5
        let command = make_command_book_with_sequence("orders", root.as_bytes().to_vec(), 0, false);

        let response = service.handle(Request::new(command)).await;
        assert!(
            response.is_err(),
            "Command with mismatched sequence should fail"
        );

        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert!(
            status.message().contains("Sequence mismatch"),
            "Error should mention sequence mismatch: {}",
            status.message()
        );
    }

    #[tokio::test]
    async fn test_handle_command_sequence_mismatch_with_auto_resequence_skips_prevalidation() {
        let (service, event_store, _, _, _) = create_default_test_service();

        // Set aggregate at sequence 5 via override
        event_store.set_next_sequence(5).await;

        let root = uuid::Uuid::new_v4();
        // Command expects sequence 0, but auto_resequence is enabled
        // Pre-validation is SKIPPED when auto_resequence is true
        // (write-time validation handles conflicts instead)
        let command = make_command_book_with_sequence("orders", root.as_bytes().to_vec(), 0, true);

        let response = service.handle(Request::new(command)).await;
        // With auto_resequence=true, pre-validation is skipped, so this should succeed
        // (MockEventStore doesn't actually validate sequences on write)
        assert!(
            response.is_ok(),
            "Command with auto_resequence should skip pre-validation: {:?}",
            response.err()
        );
    }

    #[tokio::test]
    async fn test_handle_command_missing_pages_fails() {
        let (service, _, _, _, _) = create_default_test_service();

        let root = uuid::Uuid::new_v4();
        // CommandBook with no pages
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![], // No pages
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        };

        let response = service.handle(Request::new(command)).await;
        assert!(response.is_err());

        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(
            status.message().contains("at least one page"),
            "Error should mention missing pages: {}",
            status.message()
        );
    }

    #[tokio::test]
    async fn test_handle_command_new_aggregate_sequence_zero_succeeds() {
        let (service, _, _, _, _) = create_default_test_service();

        // New aggregate - no events (next_sequence = 0)
        let root = uuid::Uuid::new_v4();
        // Command expects sequence 0 (new aggregate)
        let command = make_command_book_with_sequence("orders", root.as_bytes().to_vec(), 0, false);

        let response = service.handle(Request::new(command)).await;
        assert!(
            response.is_ok(),
            "First command to new aggregate should succeed with sequence=0"
        );
    }
}
