//! Acceptance tests using cucumber-rs (Gherkin).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;
use cucumber::{given, then, when, World};
use prost_types::Timestamp;
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use uuid::Uuid;

use evented::clients::PlaceholderBusinessLogic;
use evented::interfaces::business_client::Result as BusinessResult;
use evented::interfaces::event_bus::{BusError, EventBus, PublishResult, Result as BusResult};
use evented::interfaces::event_store::EventStore;
use evented::interfaces::snapshot_store::SnapshotStore;
use evented::interfaces::BusinessLogicClient;
use evented::proto::{
    CommandBook, ContextualCommand, Cover, EventBook, EventPage, Snapshot, Uuid as ProtoUuid,
};
use evented::services::CommandHandlerService;
use evented::storage::{SqliteEventStore, SqliteSnapshotStore};

/// Stub business logic that records calls and returns configured events.
pub struct StubBusinessLogic {
    /// Records of calls received.
    pub calls: Arc<RwLock<Vec<RecordedCall>>>,
    /// Events to return for each call.
    response_events: Arc<RwLock<Vec<EventPage>>>,
}

#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub domain: String,
    pub prior_event_count: usize,
    pub has_snapshot: bool,
    pub snapshot_sequence: Option<u32>,
}

impl Default for StubBusinessLogic {
    fn default() -> Self {
        Self {
            calls: Arc::new(RwLock::new(Vec::new())),
            response_events: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl StubBusinessLogic {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_response_events(&self, events: Vec<EventPage>) {
        *self.response_events.write().await = events;
    }

    pub async fn get_calls(&self) -> Vec<RecordedCall> {
        self.calls.read().await.clone()
    }
}

#[async_trait]
impl BusinessLogicClient for StubBusinessLogic {
    async fn handle(&self, domain: &str, command: ContextualCommand) -> BusinessResult<EventBook> {
        let prior_events = command.events.as_ref();
        let prior_event_count = prior_events.map(|e| e.pages.len()).unwrap_or(0);
        let has_snapshot = prior_events.map(|e| e.snapshot.is_some()).unwrap_or(false);
        let snapshot_sequence = prior_events
            .and_then(|e| e.snapshot.as_ref())
            .map(|s| s.sequence);

        // Record the call
        self.calls.write().await.push(RecordedCall {
            domain: domain.to_string(),
            prior_event_count,
            has_snapshot,
            snapshot_sequence,
        });

        // Build response with configured events
        let response_events = self.response_events.read().await.clone();
        let cover = command
            .command
            .as_ref()
            .and_then(|c| c.cover.clone())
            .unwrap_or_else(|| Cover {
                domain: domain.to_string(),
                root: None,
            });

        Ok(EventBook {
            cover: Some(cover),
            snapshot: None,
            pages: response_events,
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        self.domains().contains(&domain.to_string())
    }

    fn domains(&self) -> Vec<String> {
        vec!["orders".to_string(), "sagas".to_string()]
    }
}

/// Stub event bus that records published events.
pub struct StubEventBus {
    pub published: Arc<RwLock<Vec<EventBook>>>,
}

impl Default for StubEventBus {
    fn default() -> Self {
        Self {
            published: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl StubEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_published(&self) -> Vec<EventBook> {
        self.published.read().await.clone()
    }
}

#[async_trait]
impl EventBus for StubEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> BusResult<PublishResult> {
        self.published.write().await.push((*book).clone());
        Ok(PublishResult::default())
    }

    async fn subscribe(
        &self,
        _handler: Box<dyn evented::interfaces::event_bus::EventHandler>,
    ) -> BusResult<()> {
        Err(BusError::SubscribeNotSupported)
    }
}

/// Test world containing all test state.
#[derive(World)]
#[world(init = Self::new)]
pub struct TestWorld {
    #[allow(dead_code)]
    pool: SqlitePool,
    event_store: Arc<SqliteEventStore>,
    snapshot_store: Arc<SqliteSnapshotStore>,
    business_logic: Arc<StubBusinessLogic>,
    placeholder_logic: Arc<PlaceholderBusinessLogic>,
    event_bus: Arc<StubEventBus>,
    current_domain: String,
    current_aggregate: Uuid,
    #[allow(dead_code)]
    use_placeholder: bool,
    /// Last error from a rejected command.
    last_error: Option<String>,
}

impl std::fmt::Debug for TestWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestWorld")
            .field("current_domain", &self.current_domain)
            .field("current_aggregate", &self.current_aggregate)
            .finish()
    }
}

impl TestWorld {
    async fn new() -> Self {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
        let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool.clone()));
        event_store.init().await.unwrap();
        snapshot_store.init().await.unwrap();

        Self {
            pool,
            event_store,
            snapshot_store,
            business_logic: Arc::new(StubBusinessLogic::new()),
            placeholder_logic: Arc::new(PlaceholderBusinessLogic::with_defaults()),
            event_bus: Arc::new(StubEventBus::new()),
            current_domain: String::new(),
            current_aggregate: Uuid::nil(),
            use_placeholder: false,
            last_error: None,
        }
    }

    fn make_event(&self, sequence: u32, event_type: &str) -> EventPage {
        EventPage {
            sequence: Some(evented::proto::event_page::Sequence::Num(sequence)),
            created_at: Some(Timestamp {
                seconds: 1704067200 + sequence as i64,
                nanos: 0,
            }),
            event: Some(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", event_type),
                value: vec![1, 2, 3, sequence as u8],
            }),
            synchronous: false,
        }
    }

    fn parse_aggregate_id(&self, id: &str) -> Uuid {
        // Create deterministic UUID from string for testing
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        let hash = hasher.finish();

        // Build UUID from hash bytes
        let bytes: [u8; 16] = {
            let mut b = [0u8; 16];
            b[0..8].copy_from_slice(&hash.to_le_bytes());
            b[8..16].copy_from_slice(&hash.to_be_bytes());
            b
        };
        Uuid::from_bytes(bytes)
    }
}

// Step implementations

#[given("an empty event store")]
async fn given_empty_event_store(_world: &mut TestWorld) {
    // Already initialized empty in TestWorld::new()
}

#[given("a stub business logic service")]
async fn given_stub_business_logic(_world: &mut TestWorld) {
    // Already set up in TestWorld::new()
}

#[given(expr = "no prior events for aggregate {string} in domain {string}")]
async fn given_no_prior_events(world: &mut TestWorld, aggregate_id: String, domain: String) {
    world.current_domain = domain;
    world.current_aggregate = world.parse_aggregate_id(&aggregate_id);
    // No events to add - store is empty
}

#[given(expr = "prior events for aggregate {string} in domain {string}:")]
async fn given_prior_events(
    world: &mut TestWorld,
    aggregate_id: String,
    domain: String,
    step: &cucumber::gherkin::Step,
) {
    world.current_domain = domain.clone();
    world.current_aggregate = world.parse_aggregate_id(&aggregate_id);

    if let Some(table) = &step.table {
        let mut events = Vec::new();
        for row in table.rows.iter().skip(1) {
            // Skip header
            let sequence: u32 = row[0].parse().unwrap();
            let event_type = &row[1];
            events.push(world.make_event(sequence, event_type));
        }
        world
            .event_store
            .add(&domain, world.current_aggregate, events)
            .await
            .unwrap();
    }
}

#[given(expr = "a snapshot at sequence {int} for aggregate {string}")]
async fn given_snapshot(world: &mut TestWorld, sequence: u32, aggregate_id: String) {
    let root = world.parse_aggregate_id(&aggregate_id);
    let snapshot = Snapshot {
        sequence,
        state: Some(prost_types::Any {
            type_url: "type.googleapis.com/TestState".to_string(),
            value: vec![10, 20, 30],
        }),
    };
    world
        .snapshot_store
        .put(&world.current_domain, root, snapshot)
        .await
        .unwrap();
}

#[when(expr = "I send a {string} command for aggregate {string}")]
async fn when_send_command(world: &mut TestWorld, command_type: String, aggregate_id: String) {
    let root = world.parse_aggregate_id(&aggregate_id);

    // Configure stub to return one new event
    let next_seq = world
        .event_store
        .get_next_sequence(&world.current_domain, root)
        .await
        .unwrap();
    world
        .business_logic
        .set_response_events(vec![world.make_event(next_seq, &command_type)])
        .await;

    // Create command handler and send command
    let handler = CommandHandlerService::new(
        world.event_store.clone(),
        world.snapshot_store.clone(),
        world.business_logic.clone(),
        world.event_bus.clone(),
    );

    let command_book = CommandBook {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: vec![evented::proto::CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", command_type),
                value: vec![],
            }),
        }],
    };

    use evented::proto::business_coordinator_server::BusinessCoordinator;
    handler
        .handle(tonic::Request::new(command_book))
        .await
        .unwrap();
}

#[when(expr = "I send an {string} command for aggregate {string}")]
async fn when_send_an_command(world: &mut TestWorld, command_type: String, aggregate_id: String) {
    when_send_command(world, command_type, aggregate_id).await;
}

#[when(expr = "I record events directly for aggregate {string}:")]
async fn when_record_events(
    world: &mut TestWorld,
    aggregate_id: String,
    step: &cucumber::gherkin::Step,
) {
    let root = world.parse_aggregate_id(&aggregate_id);

    let mut events = Vec::new();
    if let Some(table) = &step.table {
        for row in table.rows.iter().skip(1) {
            let sequence: u32 = row[0].parse().unwrap();
            let event_type = &row[1];
            events.push(world.make_event(sequence, event_type));
        }
    }

    let handler = CommandHandlerService::new(
        world.event_store.clone(),
        world.snapshot_store.clone(),
        world.business_logic.clone(),
        world.event_bus.clone(),
    );

    let event_book = EventBook {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        snapshot: None,
        pages: events,
    };

    use evented::proto::business_coordinator_server::BusinessCoordinator;
    handler
        .record(tonic::Request::new(event_book))
        .await
        .unwrap();
}

#[then("the business logic receives the command with empty event history")]
async fn then_business_receives_empty_history(world: &mut TestWorld) {
    let calls = world.business_logic.get_calls().await;
    assert!(!calls.is_empty(), "Business logic should have been called");
    let last_call = calls.last().unwrap();
    assert_eq!(
        last_call.prior_event_count, 0,
        "Expected empty event history"
    );
}

#[then(expr = "the business logic receives the command with {int} prior events")]
async fn then_business_receives_events(world: &mut TestWorld, event_count: usize) {
    let calls = world.business_logic.get_calls().await;
    assert!(!calls.is_empty(), "Business logic should have been called");
    let last_call = calls.last().unwrap();
    assert_eq!(
        last_call.prior_event_count, event_count,
        "Expected {} prior events",
        event_count
    );
}

#[then("the business logic receives the snapshot and events from sequence 2")]
async fn then_business_receives_snapshot(world: &mut TestWorld) {
    let calls = world.business_logic.get_calls().await;
    assert!(!calls.is_empty(), "Business logic should have been called");
    let last_call = calls.last().unwrap();
    assert!(last_call.has_snapshot, "Expected snapshot to be present");
    assert_eq!(
        last_call.snapshot_sequence,
        Some(2),
        "Expected snapshot at sequence 2"
    );
}

#[then(expr = "{int} event is persisted for aggregate {string}")]
async fn then_events_persisted(world: &mut TestWorld, count: usize, aggregate_id: String) {
    let root = world.parse_aggregate_id(&aggregate_id);
    let events = world
        .event_store
        .get(&world.current_domain, root)
        .await
        .unwrap();
    assert_eq!(events.len(), count, "Expected {} persisted events", count);
}

#[then(expr = "{int} events are persisted for aggregate {string}")]
async fn then_multiple_events_persisted(world: &mut TestWorld, count: usize, aggregate_id: String) {
    then_events_persisted(world, count, aggregate_id).await;
}

#[then(expr = "{int} events total exist for aggregate {string}")]
async fn then_total_events(world: &mut TestWorld, count: usize, aggregate_id: String) {
    then_events_persisted(world, count, aggregate_id).await;
}

#[then("the event bus receives the new events")]
async fn then_event_bus_receives(world: &mut TestWorld) {
    let published = world.event_bus.get_published().await;
    assert!(
        !published.is_empty(),
        "Event bus should have received events"
    );
}

// Placeholder business logic steps

#[given(expr = "placeholder business logic for domain {string}")]
async fn given_placeholder_logic(world: &mut TestWorld, domain: String) {
    world.current_domain = domain;
    world.use_placeholder = true;
}

#[when(expr = "I send a {string} command through placeholder logic for aggregate {string}")]
async fn when_send_placeholder_command(
    world: &mut TestWorld,
    command_type: String,
    aggregate_id: String,
) {
    let root = world.parse_aggregate_id(&aggregate_id);
    world.current_aggregate = root;

    let handler = CommandHandlerService::new(
        world.event_store.clone(),
        world.snapshot_store.clone(),
        world.placeholder_logic.clone(),
        world.event_bus.clone(),
    );

    let command_book = CommandBook {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: vec![evented::proto::CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", command_type),
                value: vec![],
            }),
        }],
    };

    use evented::proto::business_coordinator_server::BusinessCoordinator;
    handler
        .handle(tonic::Request::new(command_book))
        .await
        .unwrap();
}

#[when(expr = "I send an {string} command through placeholder logic for aggregate {string}")]
async fn when_send_an_placeholder_command(
    world: &mut TestWorld,
    command_type: String,
    aggregate_id: String,
) {
    when_send_placeholder_command(world, command_type, aggregate_id).await;
}

#[then(expr = "the event type contains {string}")]
async fn then_event_type_contains(world: &mut TestWorld, expected: String) {
    let events = world
        .event_store
        .get(&world.current_domain, world.current_aggregate)
        .await
        .unwrap();

    assert!(!events.is_empty(), "Expected at least one event");

    let last_event = events.last().unwrap();
    let event_type = last_event
        .event
        .as_ref()
        .map(|e| e.type_url.clone())
        .unwrap_or_default();

    assert!(
        event_type.contains(&expected),
        "Expected event type containing '{}', got '{}'",
        expected,
        event_type
    );
}

#[then(expr = "the latest event type contains {string}")]
async fn then_latest_event_type_contains(world: &mut TestWorld, expected: String) {
    then_event_type_contains(world, expected).await;
}

#[then(expr = "the command is rejected with error containing {string}")]
async fn then_command_rejected(world: &mut TestWorld, expected_error: String) {
    let error = world
        .last_error
        .as_ref()
        .expect("Expected command to be rejected, but it succeeded");
    assert!(
        error
            .to_lowercase()
            .contains(&expected_error.to_lowercase()),
        "Expected error containing '{}', got '{}'",
        expected_error,
        error
    );
}

#[tokio::main]
async fn main() {
    TestWorld::cucumber()
        .run("tests/acceptance/features")
        .await;
}
