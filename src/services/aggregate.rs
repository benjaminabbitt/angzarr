//! Aggregate service (AggregateCoordinator).

use std::sync::Arc;

use tonic::transport::Channel;
use tonic::{Request, Response, Status};

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::orchestration::aggregate::grpc::GrpcAggregateContext;
use crate::orchestration::aggregate::{
    execute_command_pipeline, execute_command_with_retry, execute_fact_pipeline,
    parse_command_cover, AggregateContext, ClientLogic, GrpcBusinessLogic, PipelineMode,
    TemporalQuery,
};
use crate::proto::{
    business_response,
    command_handler_coordinator_service_server::CommandHandlerCoordinatorService,
    command_handler_service_client::CommandHandlerServiceClient, BusinessResponse, CommandRequest,
    CommandResponse, ContextualCommand, EventRequest, FactInjectionResponse,
    SpeculateCommandHandlerRequest,
};
use crate::proto_ext::CoverExt;
use crate::services::upcaster::Upcaster;
use crate::storage::{EventStore, SnapshotStore};
use crate::utils::retry::saga_backoff;

/// Aggregate service.
///
/// Receives commands, loads prior state, calls client logic,
/// persists new events, and notifies projectors.
///
/// Uses the shared aggregate pipeline for both async and sync operations.
pub struct AggregateService {
    event_store: Arc<dyn EventStore>,
    snapshot_store: Arc<dyn SnapshotStore>,
    business: Arc<dyn ClientLogic>,
    event_bus: Arc<dyn EventBus>,
    /// When false, snapshots are not written even if client logic returns snapshot_state.
    snapshot_write_enabled: bool,
    /// When false, snapshots are not read (for testing/debugging).
    snapshot_read_enabled: bool,
    /// Service discovery for projectors (sync operations).
    discovery: Arc<dyn ServiceDiscovery>,
    /// Upcaster for event version transformation.
    upcaster: Option<Arc<Upcaster>>,
}

impl AggregateService {
    /// Create a new aggregate service with snapshots enabled.
    pub fn new(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: CommandHandlerServiceClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business: Arc::new(GrpcBusinessLogic::new(business_client)),
            event_bus,
            snapshot_write_enabled: true,
            snapshot_read_enabled: true,
            discovery,
            upcaster: None,
        }
    }

    /// Create a new aggregate service with configurable snapshot behavior.
    pub fn with_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: CommandHandlerServiceClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business: Arc::new(GrpcBusinessLogic::new(business_client)),
            event_bus,
            snapshot_write_enabled,
            snapshot_read_enabled,
            discovery,
            upcaster: None,
        }
    }

    /// Set the upcaster for event version transformation.
    pub fn with_upcaster(mut self, upcaster: Arc<Upcaster>) -> Self {
        self.upcaster = Some(upcaster);
        self
    }

    /// Create a new aggregate service with injected business logic.
    ///
    /// This constructor accepts `Arc<dyn ClientLogic>` directly instead of a gRPC client,
    /// enabling unit testing with mock implementations.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_business_logic(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business: Arc<dyn ClientLogic>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business,
            event_bus,
            snapshot_write_enabled: true,
            snapshot_read_enabled: true,
            discovery,
            upcaster: None,
        }
    }

    /// Create with injected business logic and configurable snapshot behavior.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_business_logic_and_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business: Arc<dyn ClientLogic>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business,
            event_bus,
            snapshot_write_enabled,
            snapshot_read_enabled,
            discovery,
            upcaster: None,
        }
    }

    /// Create an async context (no sync projector calls).
    fn create_async_context(&self) -> GrpcAggregateContext {
        let mut ctx = GrpcAggregateContext::with_config(
            self.event_store.clone(),
            self.snapshot_store.clone(),
            self.discovery.clone(),
            self.event_bus.clone(),
            self.snapshot_read_enabled,
            self.snapshot_write_enabled,
        );
        if let Some(ref upcaster) = self.upcaster {
            ctx = ctx.with_upcaster(upcaster.clone());
        }
        ctx
    }

    /// Create a sync context (calls sync projectors).
    fn create_sync_context(&self, sync_mode: crate::proto::SyncMode) -> GrpcAggregateContext {
        let mut ctx = GrpcAggregateContext::with_config(
            self.event_store.clone(),
            self.snapshot_store.clone(),
            self.discovery.clone(),
            self.event_bus.clone(),
            self.snapshot_read_enabled,
            self.snapshot_write_enabled,
        )
        .with_sync_mode(sync_mode);
        if let Some(ref upcaster) = self.upcaster {
            ctx = ctx.with_upcaster(upcaster.clone());
        }
        ctx
    }
}

#[tonic::async_trait]
impl CommandHandlerCoordinatorService for AggregateService {
    /// Handle command with optional sync mode (default: async fire-and-forget).
    #[tracing::instrument(name = "aggregate.handle_command", skip_all)]
    async fn handle_command(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_request = request.into_inner();
        let sync_mode = crate::proto::SyncMode::try_from(sync_request.sync_mode)
            .unwrap_or(crate::proto::SyncMode::Async);
        let command_book = sync_request.command.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::COMMAND_REQUEST_MISSING_COMMAND)
        })?;

        // Unspecified = async (fire and forget), otherwise use sync context
        let ctx = if sync_mode == crate::proto::SyncMode::Async {
            self.create_async_context()
        } else {
            self.create_sync_context(sync_mode)
        };

        let result =
            execute_command_with_retry(&ctx, &*self.business, command_book, saga_backoff()).await;

        Ok(Response::new(result?))
    }

    /// Speculative: execute command against temporal state without persisting.
    #[tracing::instrument(name = "aggregate.handle_sync_speculative", skip_all)]
    async fn handle_sync_speculative(
        &self,
        request: Request<SpeculateCommandHandlerRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let speculate_req = request.into_inner();
        let command_book = speculate_req.command.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::SPECULATE_AGG_MISSING_COMMAND)
        })?;

        let (as_of_sequence, as_of_timestamp) = match speculate_req.point_in_time {
            Some(temporal) => match temporal.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    (Some(seq), None)
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ts)) => {
                    let ts_str = format!("{}.{}", ts.seconds, ts.nanos);
                    (None, Some(ts_str))
                }
                None => (None, None),
            },
            None => (None, None),
        };

        let ctx = self.create_async_context();

        let response = execute_command_pipeline(
            &ctx,
            &*self.business,
            command_book,
            PipelineMode::Speculative {
                as_of_sequence,
                as_of_timestamp,
            },
        )
        .await?;

        Ok(Response::new(response))
    }

    /// Handle compensation flow - returns BusinessResponse for saga compensation handling.
    ///
    /// Unlike normal HandleCommand, this returns the raw BusinessResponse so the caller
    /// can inspect revocation flags and decide how to handle (quarantine, notify, etc.).
    /// If business logic returns events, they are persisted before returning.
    #[tracing::instrument(name = "aggregate.handle_compensation", skip_all)]
    async fn handle_compensation(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let sync_request = request.into_inner();
        let sync_mode = crate::proto::SyncMode::try_from(sync_request.sync_mode)
            .unwrap_or(crate::proto::SyncMode::Async);
        let command_book = sync_request.command.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::COMMAND_REQUEST_MISSING_COMMAND)
        })?;
        let (domain, root_uuid) = parse_command_cover(&command_book)?;
        let edition = command_book.edition().to_string();
        let correlation_id =
            crate::orchestration::correlation::extract_correlation_id(&command_book)?;

        let ctx = if sync_mode == crate::proto::SyncMode::Async {
            self.create_async_context()
        } else {
            self.create_sync_context(sync_mode)
        };

        // Load prior events
        let prior_events = ctx
            .load_prior_events(&domain, &edition, root_uuid, &TemporalQuery::Current)
            .await?;

        // Transform events (upcasting)
        let prior_events = ctx.transform_events(&domain, prior_events).await?;

        // Invoke business logic
        let contextual_command = ContextualCommand {
            events: Some(prior_events.clone()),
            command: Some(command_book),
        };

        let response = self.business.invoke(contextual_command).await?;

        // If business returned events, persist them
        if let Some(business_response::Result::Events(ref events)) = response.result {
            if !events.pages.is_empty() {
                ctx.persist_events(
                    &prior_events,
                    events,
                    &domain,
                    &edition,
                    root_uuid,
                    &correlation_id,
                )
                .await?;

                // Post-persist: publish to bus
                ctx.post_persist(events).await?;
            }
        }

        Ok(Response::new(response))
    }

    /// Handle event (fact) injection - external realities that cannot be rejected.
    ///
    /// Facts are events that already happened externally and cannot be rejected by business logic.
    /// They are persisted unconditionally with coordinator-assigned sequence numbers.
    ///
    /// `route_to_handler`: When true (default), invokes the aggregate's handle_fact method
    /// for validation/error checking before persistence. The aggregate cannot reject facts,
    /// but can validate data integrity and log warnings. When false, facts are persisted
    /// directly without aggregate involvement.
    ///
    /// Idempotent: subsequent requests with same external_id return original events.
    #[tracing::instrument(name = "aggregate.handle_event", skip_all)]
    async fn handle_event(
        &self,
        request: Request<EventRequest>,
    ) -> Result<Response<FactInjectionResponse>, Status> {
        let sync_event_book = request.into_inner();
        let sync_mode = crate::proto::SyncMode::try_from(sync_event_book.sync_mode)
            .unwrap_or(crate::proto::SyncMode::Async);
        let fact_events = sync_event_book
            .events
            .ok_or_else(|| Status::invalid_argument(super::errmsg::EVENT_REQUEST_MISSING_EVENTS))?;

        let ctx = if sync_mode == crate::proto::SyncMode::Async {
            self.create_async_context()
        } else {
            self.create_sync_context(sync_mode)
        };

        // Use aggregate handler if route_to_handler is true (default behavior)
        let business: Option<&dyn ClientLogic> = if sync_event_book.route_to_handler {
            Some(&*self.business)
        } else {
            None
        };

        let fact_response = execute_fact_pipeline(&ctx, business, fact_events).await?;

        Ok(Response::new(FactInjectionResponse {
            events: Some(fact_response.events),
            already_processed: fact_response.already_processed,
            projections: fact_response.projections,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MockEventBus;
    use crate::discovery::StaticServiceDiscovery;
    use crate::orchestration::aggregate::{ClientLogic, FactContext};
    use crate::proto::{
        business_response, command_page, event_page, CommandBook, CommandPage, ContextualCommand,
        Cover, EventBook, EventPage, MergeStrategy, SyncMode, Uuid as ProtoUuid,
    };
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};
    use prost_types::Any;
    use std::collections::VecDeque;
    use tokio::sync::Mutex;
    use tonic::Status;
    use uuid::Uuid;

    // ============================================================================
    // Mock ClientLogic Implementation
    // ============================================================================

    /// Mock business logic for testing.
    ///
    /// Returns pre-configured responses from a queue.
    struct MockClientLogic {
        responses: Mutex<VecDeque<Result<BusinessResponse, Status>>>,
        fact_responses: Mutex<VecDeque<Result<EventBook, Status>>>,
        invocations: Mutex<Vec<ContextualCommand>>,
    }

    impl MockClientLogic {
        fn new() -> Self {
            Self {
                responses: Mutex::new(VecDeque::new()),
                fact_responses: Mutex::new(VecDeque::new()),
                invocations: Mutex::new(Vec::new()),
            }
        }

        async fn enqueue_response(&self, response: Result<BusinessResponse, Status>) {
            self.responses.lock().await.push_back(response);
        }

        async fn enqueue_events(&self, events: EventBook) {
            let response = BusinessResponse {
                result: Some(business_response::Result::Events(events)),
            };
            self.enqueue_response(Ok(response)).await;
        }

        async fn enqueue_fact_response(&self, response: Result<EventBook, Status>) {
            self.fact_responses.lock().await.push_back(response);
        }
    }

    #[async_trait::async_trait]
    impl ClientLogic for MockClientLogic {
        async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
            self.invocations.lock().await.push(cmd);
            self.responses.lock().await.pop_front().unwrap_or_else(|| {
                Ok(BusinessResponse {
                    result: Some(business_response::Result::Events(EventBook::default())),
                })
            })
        }

        async fn invoke_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
            self.fact_responses
                .lock()
                .await
                .pop_front()
                .unwrap_or_else(|| Ok(ctx.facts))
        }
    }

    // ============================================================================
    // Test Helpers
    // ============================================================================

    fn make_proto_uuid(u: Uuid) -> ProtoUuid {
        ProtoUuid {
            value: u.as_bytes().to_vec(),
        }
    }

    fn make_cover(domain: &str, root: Uuid) -> Cover {
        Cover {
            domain: domain.to_string(),
            root: Some(make_proto_uuid(root)),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }
    }

    fn make_command_book(domain: &str, root: Uuid, sequence: u32) -> CommandBook {
        CommandBook {
            cover: Some(make_cover(domain, root)),
            pages: vec![CommandPage {
                sequence,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "test.Command".to_string(),
                    value: vec![],
                })),
                merge_strategy: MergeStrategy::MergeCommutative as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_event_page(seq: u32) -> EventPage {
        EventPage {
            sequence_type: Some(event_page::SequenceType::Sequence(seq)),
            payload: Some(event_page::Payload::Event(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            })),
            created_at: None,
        }
    }

    fn make_fact_page() -> EventPage {
        use crate::proto::FactSequence;
        EventPage {
            sequence_type: Some(event_page::SequenceType::Fact(FactSequence {
                source: "test".to_string(),
                description: "Test fact".to_string(),
            })),
            payload: Some(event_page::Payload::Event(Any {
                type_url: "test.Fact".to_string(),
                value: vec![],
            })),
            created_at: None,
        }
    }

    fn make_event_book(domain: &str, root: Uuid, pages: Vec<EventPage>) -> EventBook {
        EventBook {
            cover: Some(make_cover(domain, root)),
            pages,
            snapshot: None,
            ..Default::default()
        }
    }

    async fn create_test_service() -> (AggregateService, Arc<MockClientLogic>) {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let business = Arc::new(MockClientLogic::new());
        let event_bus = Arc::new(MockEventBus::new());
        let discovery = Arc::new(StaticServiceDiscovery::new());

        let service = AggregateService::with_business_logic(
            event_store,
            snapshot_store,
            business.clone(),
            event_bus,
            discovery,
        );

        (service, business)
    }

    // ============================================================================
    // Constructor Tests
    // ============================================================================

    #[tokio::test]
    async fn test_with_business_logic_creates_service() {
        let (service, _) = create_test_service().await;
        // Verify service is created with expected configuration
        assert!(service.snapshot_read_enabled);
        assert!(service.snapshot_write_enabled);
        assert!(service.upcaster.is_none());
    }

    #[tokio::test]
    async fn test_with_business_logic_and_config_respects_snapshot_settings() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let business: Arc<dyn ClientLogic> = Arc::new(MockClientLogic::new());
        let event_bus = Arc::new(MockEventBus::new());
        let discovery = Arc::new(StaticServiceDiscovery::new());

        let service = AggregateService::with_business_logic_and_config(
            event_store,
            snapshot_store,
            business,
            event_bus,
            discovery,
            false, // snapshot_read_enabled
            false, // snapshot_write_enabled
        );

        assert!(!service.snapshot_read_enabled);
        assert!(!service.snapshot_write_enabled);
    }

    // ============================================================================
    // handle_command Tests
    // ============================================================================

    #[tokio::test]
    async fn test_handle_command_invokes_business_logic() {
        let (service, business) = create_test_service().await;

        let root = Uuid::new_v4();
        let command_book = make_command_book("orders", root, 0);
        let events = make_event_book("orders", root, vec![make_event_page(0)]);
        business.enqueue_events(events).await;

        let request = Request::new(CommandRequest {
            command: Some(command_book),
            sync_mode: SyncMode::Async as i32,
        });

        let response = service.handle_command(request).await;
        assert!(response.is_ok());

        // Verify business logic was invoked
        let invocations = business.invocations.lock().await;
        assert_eq!(invocations.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_command_missing_command_returns_error() {
        let (service, _) = create_test_service().await;

        let request = Request::new(CommandRequest {
            command: None,
            sync_mode: SyncMode::Async as i32,
        });

        let response = service.handle_command(request).await;
        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_handle_command_with_sync_mode_creates_sync_context() {
        let (service, business) = create_test_service().await;

        let root = Uuid::new_v4();
        let command_book = make_command_book("orders", root, 0);
        let events = make_event_book("orders", root, vec![make_event_page(0)]);
        business.enqueue_events(events).await;

        let request = Request::new(CommandRequest {
            command: Some(command_book),
            sync_mode: SyncMode::Simple as i32,
        });

        let response = service.handle_command(request).await;
        assert!(response.is_ok());
    }

    // ============================================================================
    // handle_sync_speculative Tests
    // ============================================================================

    #[tokio::test]
    async fn test_handle_sync_speculative_missing_command_returns_error() {
        let (service, _) = create_test_service().await;

        let request = Request::new(SpeculateCommandHandlerRequest {
            command: None,
            point_in_time: None,
        });

        let response = service.handle_sync_speculative(request).await;
        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_handle_sync_speculative_with_as_of_sequence() {
        let (service, business) = create_test_service().await;

        let root = Uuid::new_v4();
        let command_book = make_command_book("orders", root, 0);
        let events = make_event_book("orders", root, vec![make_event_page(0)]);
        business.enqueue_events(events).await;

        let request = Request::new(SpeculateCommandHandlerRequest {
            command: Some(command_book),
            point_in_time: Some(crate::proto::TemporalQuery {
                point_in_time: Some(crate::proto::temporal_query::PointInTime::AsOfSequence(5)),
            }),
        });

        let response = service.handle_sync_speculative(request).await;
        assert!(response.is_ok());
    }

    // Note: test_handle_sync_speculative_with_as_of_time was removed because
    // MockEventStore doesn't implement get_until_timestamp properly. The actual
    // timestamp-based queries are tested in the storage interface tests.

    // ============================================================================
    // handle_compensation Tests
    // ============================================================================

    #[tokio::test]
    async fn test_handle_compensation_missing_command_returns_error() {
        let (service, _) = create_test_service().await;

        let request = Request::new(CommandRequest {
            command: None,
            sync_mode: SyncMode::Async as i32,
        });

        let response = service.handle_compensation(request).await;
        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_handle_compensation_returns_business_response() {
        let (service, business) = create_test_service().await;

        let root = Uuid::new_v4();
        let command_book = make_command_book("orders", root, 0);
        let events = make_event_book("orders", root, vec![make_event_page(0)]);
        business.enqueue_events(events).await;

        let request = Request::new(CommandRequest {
            command: Some(command_book),
            sync_mode: SyncMode::Async as i32,
        });

        let response = service.handle_compensation(request).await;
        assert!(response.is_ok());
        let br = response.unwrap().into_inner();
        assert!(br.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_compensation_with_empty_response() {
        let (service, business) = create_test_service().await;

        let root = Uuid::new_v4();
        let command_book = make_command_book("orders", root, 0);
        // Default response is empty events
        business.enqueue_events(EventBook::default()).await;

        let request = Request::new(CommandRequest {
            command: Some(command_book),
            sync_mode: SyncMode::Async as i32,
        });

        let response = service.handle_compensation(request).await;
        assert!(response.is_ok());
        let br = response.unwrap().into_inner();
        // Verify events result was returned
        match br.result {
            Some(business_response::Result::Events(_)) => {}
            _ => panic!("Expected events response"),
        }
    }

    // ============================================================================
    // handle_event (Fact Injection) Tests
    // ============================================================================

    #[tokio::test]
    async fn test_handle_event_missing_events_returns_error() {
        let (service, _) = create_test_service().await;

        let request = Request::new(EventRequest {
            events: None,
            sync_mode: SyncMode::Async as i32,
            route_to_handler: true,
        });

        let response = service.handle_event(request).await;
        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_handle_event_with_route_to_handler() {
        let (service, business) = create_test_service().await;

        let root = Uuid::new_v4();
        let facts = make_event_book("orders", root, vec![make_fact_page()]);
        business.enqueue_fact_response(Ok(facts.clone())).await;

        let request = Request::new(EventRequest {
            events: Some(facts),
            sync_mode: SyncMode::Async as i32,
            route_to_handler: true,
        });

        let response = service.handle_event(request).await;
        assert!(
            response.is_ok(),
            "Expected ok but got: {:?}",
            response.err()
        );
        let fact_response = response.unwrap().into_inner();
        assert!(fact_response.events.is_some());
    }

    #[tokio::test]
    async fn test_handle_event_without_route_to_handler() {
        let (service, _) = create_test_service().await;

        let root = Uuid::new_v4();
        let facts = make_event_book("orders", root, vec![make_fact_page()]);

        let request = Request::new(EventRequest {
            events: Some(facts),
            sync_mode: SyncMode::Async as i32,
            route_to_handler: false,
        });

        let response = service.handle_event(request).await;
        assert!(
            response.is_ok(),
            "Expected ok but got: {:?}",
            response.err()
        );
    }

    // ============================================================================
    // Context Creation Tests
    // ============================================================================

    #[tokio::test]
    async fn test_create_async_context_succeeds() {
        let (service, _) = create_test_service().await;
        // Verify context creation doesn't panic
        let _ctx = service.create_async_context();
    }

    #[tokio::test]
    async fn test_create_sync_context_succeeds() {
        let (service, _) = create_test_service().await;
        // Verify context creation with sync mode doesn't panic
        let _ctx = service.create_sync_context(SyncMode::Simple);
    }
}
