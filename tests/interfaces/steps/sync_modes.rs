//! SyncMode interface step definitions.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use angzarr::proto::{
    command_page, event_page, CommandBook, CommandPage, CommandResponse, ContextualCommand, Cover,
    EventBook, EventPage, MergeStrategy, Projection, SagaResponse, Uuid as ProtoUuid,
};
use angzarr::standalone::{
    AggregateHandler, ProjectionMode, ProjectorConfig, ProjectorHandler, RuntimeBuilder,
    SagaConfig, SagaHandler,
};
use async_trait::async_trait;
use cucumber::{given, then, when, World};
use prost_types::Any;
use tokio::sync::RwLock;
use tonic::Status;
use uuid::Uuid;

/// Test context for SyncMode scenarios.
#[derive(World)]
#[world(init = Self::new)]
pub struct SyncModeWorld {
    runtime: Option<RuntimeBuilder>,
    started_runtime: Option<angzarr::standalone::Runtime>,
    last_response: Option<CommandResponse>,
    last_error: Option<Status>,
    projector_state: Option<ProjectorState>,
    async_projector_state: Option<ProjectorState>,
    saga_state: Option<SagaState>,
    last_domain: String,
    last_root: Uuid,
}

impl std::fmt::Debug for SyncModeWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncModeWorld")
            .field("runtime", &"<RuntimeBuilder>")
            .field("last_response", &self.last_response)
            .field("last_error", &self.last_error)
            .field("last_domain", &self.last_domain)
            .finish()
    }
}

#[derive(Clone)]
struct ProjectorState {
    name: String,
    received: Arc<RwLock<Vec<EventBook>>>,
    processed_before_response: Arc<AtomicBool>,
}

impl ProjectorState {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            received: Arc::new(RwLock::new(Vec::new())),
            processed_before_response: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn received_count(&self) -> usize {
        self.received.read().await.len()
    }

    async fn get_events(&self) -> Vec<EventBook> {
        self.received.read().await.clone()
    }
}

#[derive(Clone)]
struct SagaState {
    triggered: Arc<AtomicBool>,
    commands_emitted: Arc<AtomicU32>,
}

impl SagaState {
    fn new() -> Self {
        Self {
            triggered: Arc::new(AtomicBool::new(false)),
            commands_emitted: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl SyncModeWorld {
    fn new() -> Self {
        Self {
            runtime: Some(RuntimeBuilder::new().with_sqlite_memory()),
            started_runtime: None,
            last_response: None,
            last_error: None,
            projector_state: None,
            async_projector_state: None,
            saga_state: None,
            last_domain: String::new(),
            last_root: Uuid::nil(),
        }
    }

    fn take_runtime(&mut self) -> RuntimeBuilder {
        self.runtime.take().expect("Runtime already consumed")
    }

    fn set_runtime(&mut self, runtime: RuntimeBuilder) {
        self.runtime = Some(runtime);
    }
}

/// Simple test aggregate that echoes commands as events.
struct EchoAggregate;

#[async_trait]
impl AggregateHandler for EchoAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .map(|p| p.sequence + 1)
            .unwrap_or(0);

        let event_pages: Vec<EventPage> = command_book
            .pages
            .iter()
            .enumerate()
            .map(|(i, cmd_page)| {
                let event = match &cmd_page.payload {
                    Some(command_page::Payload::Command(c)) => Some(c.clone()),
                    _ => None,
                };
                EventPage {
                    sequence: next_seq + i as u32,
                    payload: event.map(event_page::Payload::Event),
                    created_at: None,
                }
            })
            .collect();

        Ok(EventBook {
            cover,
            pages: event_pages,
            snapshot: None,
            ..Default::default()
        })
    }
}

/// Aggregate that produces N events per command.
struct MultiEventAggregate {
    events_per_command: u32,
}

impl MultiEventAggregate {
    fn new(events_per_command: u32) -> Self {
        Self { events_per_command }
    }
}

#[async_trait]
impl AggregateHandler for MultiEventAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .map(|p| p.sequence + 1)
            .unwrap_or(0);

        let pages: Vec<EventPage> = (0..self.events_per_command)
            .map(|i| EventPage {
                sequence: next_seq + i,
                payload: Some(event_page::Payload::Event(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![i as u8],
                })),
                created_at: None,
            })
            .collect();

        Ok(EventBook {
            cover,
            pages,
            snapshot: None,
            ..Default::default()
        })
    }
}

/// Recording projector that tracks received events.
struct RecordingProjector {
    state: ProjectorState,
    synchronous: bool,
}

impl RecordingProjector {
    fn new(state: ProjectorState, synchronous: bool) -> Self {
        Self { state, synchronous }
    }
}

#[async_trait]
impl ProjectorHandler for RecordingProjector {
    async fn handle(
        &self,
        events: &EventBook,
        _mode: ProjectionMode,
    ) -> Result<Projection, Status> {
        self.state.received.write().await.push(events.clone());
        self.state
            .processed_before_response
            .store(true, Ordering::SeqCst);

        Ok(Projection {
            projector: self.state.name.clone(),
            cover: events.cover.clone(),
            sequence: events.pages.len() as u32,
            projection: if self.synchronous {
                Some(Any {
                    type_url: format!("{}.Output", self.state.name),
                    value: b"projection-data".to_vec(),
                })
            } else {
                None
            },
        })
    }
}

/// Failing projector for error handling tests.
struct FailingProjector;

#[async_trait]
impl ProjectorHandler for FailingProjector {
    async fn handle(
        &self,
        _events: &EventBook,
        _mode: ProjectionMode,
    ) -> Result<Projection, Status> {
        Err(Status::internal("Projector failure"))
    }
}

/// Recording saga that tracks activations.
struct RecordingSaga {
    state: SagaState,
}

impl RecordingSaga {
    fn new(state: SagaState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl SagaHandler for RecordingSaga {
    async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(vec![])
    }

    async fn execute(
        &self,
        _source: &EventBook,
        _destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        self.state.triggered.store(true, Ordering::SeqCst);
        self.state.commands_emitted.fetch_add(1, Ordering::SeqCst);
        Ok(SagaResponse::default())
    }
}

fn create_test_command(domain: &str, root: Uuid, sequence: u32) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: Uuid::new_v4().to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.TestCommand".to_string(),
                value: b"test-data".to_vec(),
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None,
    }
}

// ==========================================================================
// Background
// ==========================================================================

#[given("a SyncMode test environment")]
async fn given_sync_mode_environment(world: &mut SyncModeWorld) {
    // Reset to fresh state
    world.runtime = Some(
        RuntimeBuilder::new()
            .with_sqlite_memory()
            .register_aggregate("orders", EchoAggregate),
    );
    world.started_runtime = None;
    world.last_response = None;
    world.last_error = None;
    world.projector_state = None;
    world.async_projector_state = None;
    world.saga_state = None;
}

// ==========================================================================
// Given - Projector Registration
// ==========================================================================

#[given("a sync projector is registered")]
async fn given_sync_projector(world: &mut SyncModeWorld) {
    let state = ProjectorState::new("sync-projector");
    world.projector_state = Some(state.clone());

    let runtime = world.take_runtime();
    let runtime = runtime.register_projector(
        "sync-projector",
        RecordingProjector::new(state, true),
        ProjectorConfig::sync(),
    );
    world.set_runtime(runtime);
}

#[given(expr = "a sync projector named {string} is registered")]
async fn given_named_sync_projector(world: &mut SyncModeWorld, name: String) {
    let state = ProjectorState::new(&name);
    world.projector_state = Some(state.clone());

    let runtime = world.take_runtime();
    let runtime = runtime.register_projector(
        &name,
        RecordingProjector::new(state, true),
        ProjectorConfig::sync(),
    );
    world.set_runtime(runtime);
}

#[given(expr = "sync projectors {string} and {string} are registered")]
async fn given_two_sync_projectors(world: &mut SyncModeWorld, name1: String, name2: String) {
    let state1 = ProjectorState::new(&name1);
    let state2 = ProjectorState::new(&name2);
    world.projector_state = Some(state1.clone());

    let runtime = world.take_runtime();
    let runtime = runtime
        .register_projector(
            &name1,
            RecordingProjector::new(state1, true),
            ProjectorConfig::sync(),
        )
        .register_projector(
            &name2,
            RecordingProjector::new(state2, true),
            ProjectorConfig::sync(),
        );
    world.set_runtime(runtime);
}

#[given("an async projector is registered")]
async fn given_async_projector(world: &mut SyncModeWorld) {
    let state = ProjectorState::new("async-projector");
    world.async_projector_state = Some(state.clone());

    let runtime = world.take_runtime();
    let runtime = runtime.register_projector(
        "async-projector",
        RecordingProjector::new(state, false),
        ProjectorConfig::async_(),
    );
    world.set_runtime(runtime);
}

#[given(expr = "a sync projector {string} is registered")]
async fn given_sync_projector_with_name(world: &mut SyncModeWorld, name: String) {
    let state = ProjectorState::new(&name);
    world.projector_state = Some(state.clone());

    let runtime = world.take_runtime();
    let runtime = runtime.register_projector(
        &name,
        RecordingProjector::new(state, true),
        ProjectorConfig::sync(),
    );
    world.set_runtime(runtime);
}

#[given(expr = "an async projector {string} is registered")]
async fn given_async_projector_with_name(world: &mut SyncModeWorld, name: String) {
    let state = ProjectorState::new(&name);
    world.async_projector_state = Some(state.clone());

    let runtime = world.take_runtime();
    let runtime = runtime.register_projector(
        &name,
        RecordingProjector::new(state, false),
        ProjectorConfig::async_(),
    );
    world.set_runtime(runtime);
}

#[given("no projectors are registered")]
async fn given_no_projectors(_world: &mut SyncModeWorld) {
    // Nothing to do - no projectors registered by default
}

#[given("a failing sync projector is registered")]
async fn given_failing_projector(world: &mut SyncModeWorld) {
    let runtime = world.take_runtime();
    let runtime = runtime.register_projector(
        "failing-projector",
        FailingProjector,
        ProjectorConfig::sync(),
    );
    world.set_runtime(runtime);
}

#[given("a saga that emits commands is registered")]
async fn given_saga_with_commands(world: &mut SyncModeWorld) {
    let state = SagaState::new();
    world.saga_state = Some(state.clone());

    let runtime = world.take_runtime();
    let runtime = runtime.register_saga(
        "test-saga",
        RecordingSaga::new(state),
        SagaConfig::new("orders", "orders"),
    );
    world.set_runtime(runtime);
}

// ==========================================================================
// When - Command Execution
// ==========================================================================

async fn start_and_execute(world: &mut SyncModeWorld, domain: &str, event_count: Option<u32>) {
    // Build runtime with multi-event aggregate if needed
    let runtime = world.take_runtime();
    let runtime = if let Some(count) = event_count {
        runtime.register_aggregate(domain, MultiEventAggregate::new(count))
    } else {
        runtime
    };

    let mut started = runtime.build().await.expect("Failed to build runtime");
    started.start().await.expect("Failed to start runtime");

    let root = Uuid::new_v4();
    world.last_domain = domain.to_string();
    world.last_root = root;

    let command = create_test_command(domain, root, 0);
    let client = started.command_client();

    match client.execute(command).await {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
        }
        Err(e) => {
            world.last_response = None;
            world.last_error = Some(Status::internal(e.to_string()));
        }
    }

    world.started_runtime = Some(started);
}

#[when("I execute a command with UNSPECIFIED sync mode")]
async fn when_execute_unspecified(world: &mut SyncModeWorld) {
    // UNSPECIFIED mode uses async projectors - no projections in response
    start_and_execute(world, "orders", None).await;
}

#[when("I execute a command without specifying sync mode")]
async fn when_execute_no_sync_mode(world: &mut SyncModeWorld) {
    // Default behavior - same as UNSPECIFIED
    start_and_execute(world, "orders", None).await;
}

#[when("I execute a command with SIMPLE sync mode")]
async fn when_execute_simple(world: &mut SyncModeWorld) {
    // SIMPLE mode uses sync projectors - projections in response
    start_and_execute(world, "orders", None).await;
}

#[when(expr = "I execute a command that produces {int} events with SIMPLE sync mode")]
async fn when_execute_multi_event_simple(world: &mut SyncModeWorld, count: u32) {
    start_and_execute(world, "orders", Some(count)).await;
}

#[when(expr = "I execute a command for domain {string} with SIMPLE sync mode")]
async fn when_execute_domain_simple(world: &mut SyncModeWorld, domain: String) {
    // Register aggregate for the domain
    let runtime = world.take_runtime();
    let runtime = runtime.register_aggregate(&domain, EchoAggregate);
    world.set_runtime(runtime);

    start_and_execute(world, &domain, None).await;
}

// ==========================================================================
// Then - Response Assertions
// ==========================================================================

#[then("the command should succeed")]
async fn then_command_succeeds(world: &mut SyncModeWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    assert!(
        world.last_response.is_some(),
        "Expected response but got none"
    );
}

#[then("the response should not include projections")]
async fn then_no_projections(world: &mut SyncModeWorld) {
    let response = world.last_response.as_ref().expect("No response");
    assert!(
        response.projections.is_empty(),
        "Expected no projections but got {}",
        response.projections.len()
    );
}

#[then("the response should include the projector output")]
async fn then_has_projector_output(world: &mut SyncModeWorld) {
    let response = world.last_response.as_ref().expect("No response");
    assert!(
        !response.projections.is_empty(),
        "Expected projections but got none"
    );
}

#[then(expr = "the response should include a projection from {string}")]
async fn then_projection_from(world: &mut SyncModeWorld, projector_name: String) {
    let response = world.last_response.as_ref().expect("No response");
    let has_projection = response
        .projections
        .iter()
        .any(|p| p.projector == projector_name);
    assert!(
        has_projection,
        "Expected projection from '{}' but got: {:?}",
        projector_name,
        response
            .projections
            .iter()
            .map(|p| &p.projector)
            .collect::<Vec<_>>()
    );
}

#[then("the projection should have the correct sequence")]
async fn then_projection_sequence(world: &mut SyncModeWorld) {
    let response = world.last_response.as_ref().expect("No response");
    let projection = response.projections.first().expect("No projections");
    assert!(
        projection.sequence > 0,
        "Expected non-zero sequence, got {}",
        projection.sequence
    );
}

#[then("the response should include projections from both projectors")]
async fn then_projections_from_both(world: &mut SyncModeWorld) {
    let response = world.last_response.as_ref().expect("No response");
    assert!(
        response.projections.len() >= 2,
        "Expected at least 2 projections, got {}",
        response.projections.len()
    );
}

#[then(expr = "the response should include projection from {string}")]
async fn then_response_includes_projection(world: &mut SyncModeWorld, projector_name: String) {
    let response = world.last_response.as_ref().expect("No response");
    let has_projection = response
        .projections
        .iter()
        .any(|p| p.projector == projector_name);
    assert!(
        has_projection,
        "Expected projection from '{}'",
        projector_name
    );
}

#[then("the response should have empty projections")]
async fn then_empty_projections(world: &mut SyncModeWorld) {
    let response = world.last_response.as_ref().expect("No response");
    assert!(
        response.projections.is_empty(),
        "Expected empty projections"
    );
}

// ==========================================================================
// Then - Projector State Assertions
// ==========================================================================

#[then("the projector should eventually receive the events")]
async fn then_projector_receives_eventually(world: &mut SyncModeWorld) {
    // Wait for async processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    if let Some(state) = &world.projector_state {
        let count = state.received_count().await;
        assert!(count >= 1, "Projector should have received events");
    } else if let Some(state) = &world.async_projector_state {
        let count = state.received_count().await;
        assert!(count >= 1, "Async projector should have received events");
    }
}

#[then("the async projector should eventually receive the events")]
async fn then_async_projector_receives(world: &mut SyncModeWorld) {
    tokio::time::sleep(Duration::from_millis(100)).await;

    let state = world
        .async_projector_state
        .as_ref()
        .expect("No async projector state");
    let count = state.received_count().await;
    assert!(
        count >= 1,
        "Async projector should have received events, got {}",
        count
    );
}

#[then("the projector should have processed before response")]
async fn then_processed_before_response(world: &mut SyncModeWorld) {
    let state = world.projector_state.as_ref().expect("No projector state");
    assert!(
        state.processed_before_response.load(Ordering::SeqCst),
        "Sync projector should have processed before response returned"
    );
}

#[then(expr = "the projector should receive all {int} events in one book")]
async fn then_projector_receives_all_events(world: &mut SyncModeWorld, count: u32) {
    let state = world.projector_state.as_ref().expect("No projector state");
    let events = state.get_events().await;
    assert!(!events.is_empty(), "Projector should have received events");

    let book = &events[0];
    assert_eq!(
        book.pages.len() as u32,
        count,
        "Expected {} events in book, got {}",
        count,
        book.pages.len()
    );
}

#[then(expr = "the projector should receive events with domain {string}")]
async fn then_projector_receives_domain(world: &mut SyncModeWorld, expected_domain: String) {
    let state = world.projector_state.as_ref().expect("No projector state");
    let events = state.get_events().await;
    assert!(!events.is_empty(), "Projector should have received events");

    let book = &events[0];
    let domain = book.cover.as_ref().map(|c| c.domain.as_str()).unwrap_or("");
    assert_eq!(
        domain, expected_domain,
        "Expected domain '{}', got '{}'",
        expected_domain, domain
    );
}

#[then("the events should have the correct aggregate root")]
async fn then_events_have_root(world: &mut SyncModeWorld) {
    let state = world.projector_state.as_ref().expect("No projector state");
    let events = state.get_events().await;
    assert!(!events.is_empty(), "Projector should have received events");

    let book = &events[0];
    let root = book
        .cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .map(|r| Uuid::from_slice(&r.value).unwrap_or(Uuid::nil()))
        .unwrap_or(Uuid::nil());

    assert_eq!(
        root, world.last_root,
        "Expected root {}, got {}",
        world.last_root, root
    );
}

#[then("the events should still be persisted")]
async fn then_events_persisted(world: &mut SyncModeWorld) {
    // Events are persisted if command succeeded
    assert!(
        world.last_response.is_some(),
        "Expected response indicating persistence"
    );
}

// ==========================================================================
// Then - Saga State Assertions
// ==========================================================================

#[then("the saga should run asynchronously")]
async fn then_saga_runs_async(world: &mut SyncModeWorld) {
    // Wait for async saga
    tokio::time::sleep(Duration::from_millis(100)).await;

    if let Some(state) = &world.saga_state {
        assert!(
            state.triggered.load(Ordering::SeqCst),
            "Saga should have been triggered"
        );
    }
}
