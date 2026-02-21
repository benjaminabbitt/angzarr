//! EventQueryService interface step definitions.

use std::collections::HashMap;

use angzarr::orchestration::aggregate::DEFAULT_EDITION;
use angzarr::proto::query::Selection;
use angzarr::proto::temporal_query::PointInTime;
use angzarr::proto::{
    event_page, event_query_service_server::EventQueryService as EventQueryTrait, AggregateRoot,
    Cover, EventBook, EventPage, Query, SequenceRange, TemporalQuery, Uuid as ProtoUuid,
};
use angzarr::services::event_query::EventQueryService;
use angzarr::storage::{EventStore, SnapshotStore};
use cucumber::{gherkin::Step, given, then, when, World};
use prost_types::{Any, Timestamp};
use tokio_stream::StreamExt;
use tonic::Request;
use uuid::Uuid;

use crate::backend::{StorageBackend, StorageContext};

/// Test context for EventQueryService scenarios.
#[derive(World)]
#[world(init = Self::new)]
pub struct EventQueryServiceWorld {
    backend: StorageBackend,
    context: Option<StorageContext>,
    service: Option<EventQueryService>,
    current_domain: String,
    current_root: Uuid,
    aggregates: HashMap<String, AggregateState>,
    last_event_book: Option<EventBook>,
    last_event_books: Vec<EventBook>,
    last_roots: Vec<AggregateRoot>,
    last_error: Option<tonic::Status>,
}

impl std::fmt::Debug for EventQueryServiceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventQueryServiceWorld")
            .field("backend", &self.backend)
            .field("context", &self.context)
            .field("service", &"<EventQueryService>")
            .field("current_domain", &self.current_domain)
            .field("current_root", &self.current_root)
            .field("aggregates", &self.aggregates)
            .field("last_event_book", &"<EventBook>")
            .field(
                "last_event_books",
                &format!("[{} books]", self.last_event_books.len()),
            )
            .field("last_roots", &format!("[{} roots]", self.last_roots.len()))
            .field("last_error", &self.last_error)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
struct AggregateState {
    domain: String,
    root: Uuid,
    event_count: u32,
    correlation_id: String,
}

impl EventQueryServiceWorld {
    fn new() -> Self {
        Self {
            backend: StorageBackend::from_env(),
            context: None,
            service: None,
            current_domain: String::new(),
            current_root: Uuid::nil(),
            aggregates: HashMap::new(),
            last_event_book: None,
            last_event_books: Vec::new(),
            last_roots: Vec::new(),
            last_error: None,
        }
    }

    fn service(&self) -> &EventQueryService {
        self.service.as_ref().expect("Service not initialized")
    }

    fn event_store(&self) -> &dyn EventStore {
        self.context
            .as_ref()
            .expect("Storage context not initialized")
            .event_store
            .as_ref()
    }

    fn snapshot_store(&self) -> &dyn SnapshotStore {
        self.context
            .as_ref()
            .expect("Storage context not initialized")
            .snapshot_store
            .as_ref()
    }

    fn make_event_page(&self, seq: u32, type_url: &str, payload: Vec<u8>) -> EventPage {
        EventPage {
            sequence: seq,
            created_at: None,
            payload: Some(event_page::Payload::Event(Any {
                type_url: type_url.to_string(),
                value: payload,
            })),
        }
    }

    fn make_event_page_with_timestamp(
        &self,
        seq: u32,
        type_url: &str,
        timestamp: Timestamp,
    ) -> EventPage {
        EventPage {
            sequence: seq,
            created_at: Some(timestamp),
            payload: Some(event_page::Payload::Event(Any {
                type_url: type_url.to_string(),
                value: vec![seq as u8],
            })),
        }
    }

    fn agg_key(&self, domain: &str, root: Uuid) -> String {
        format!("{}:{}", domain, root)
    }

    fn make_query(&self, domain: &str, root: Uuid) -> Query {
        Query {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            selection: None,
        }
    }
}

// ==========================================================================
// Background
// ==========================================================================

#[given("an EventQueryService backend")]
async fn given_event_query_service_backend(world: &mut EventQueryServiceWorld) {
    let context = StorageContext::new(world.backend).await;
    let service =
        EventQueryService::new(context.event_store.clone(), context.snapshot_store.clone());

    world.context = Some(context);
    world.service = Some(service);
}

// ==========================================================================
// Given steps - Aggregate setup
// ==========================================================================

#[given(expr = "an aggregate {string} with {int} events")]
async fn given_aggregate_with_events(
    world: &mut EventQueryServiceWorld,
    domain: String,
    count: u32,
) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let mut pages = Vec::new();
    for seq in 0..count {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        world
            .event_store()
            .add(&domain, DEFAULT_EDITION, root, pages, "")
            .await
            .expect("Failed to add events");
    }

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            event_count: count,
            correlation_id: String::new(),
        },
    );
}

#[given(expr = "no events exist for domain {string} and root {string}")]
async fn given_no_events_for_root(
    world: &mut EventQueryServiceWorld,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain;
    world.current_root = root;
    // No events added - just set up the context
}

#[given(expr = "an aggregate {string} with events at sequences {int}, {int}, {int}, {int}, {int}")]
async fn given_aggregate_with_sequences(
    world: &mut EventQueryServiceWorld,
    domain: String,
    s0: u32,
    s1: u32,
    s2: u32,
    s3: u32,
    s4: u32,
) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let sequences = [s0, s1, s2, s3, s4];
    let mut pages = Vec::new();
    for seq in sequences {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    world
        .event_store()
        .add(&domain, DEFAULT_EDITION, root, pages, "")
        .await
        .expect("Failed to add events");

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            event_count: 5,
            correlation_id: String::new(),
        },
    );
}

#[given(expr = "an aggregate {string} with events at timestamps:")]
async fn given_aggregate_with_timestamps(
    world: &mut EventQueryServiceWorld,
    step: &Step,
    domain: String,
) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let mut pages = Vec::new();

    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            // Skip header row
            let seq: u32 = row[0].parse().expect("Invalid sequence");
            let timestamp_str = &row[1];

            // Parse ISO timestamp
            let timestamp = parse_iso_timestamp(timestamp_str);

            pages.push(world.make_event_page_with_timestamp(
                seq,
                &format!("type.test/Event{}", seq),
                timestamp,
            ));
        }
    }

    let event_count = pages.len() as u32;

    world
        .event_store()
        .add(&domain, DEFAULT_EDITION, root, pages, "")
        .await
        .expect("Failed to add events");

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            event_count,
            correlation_id: String::new(),
        },
    );
}

#[given(expr = "an aggregate {string} with correlation ID {string} and {int} events")]
async fn given_aggregate_with_correlation(
    world: &mut EventQueryServiceWorld,
    domain: String,
    correlation_id: String,
    count: u32,
) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let mut pages = Vec::new();
    for seq in 0..count {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        world
            .event_store()
            .add(&domain, DEFAULT_EDITION, root, pages, &correlation_id)
            .await
            .expect("Failed to add events");
    }

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            event_count: count,
            correlation_id,
        },
    );
}

#[given(expr = "an aggregate {string} with {int} events and a snapshot at sequence {int}")]
async fn given_aggregate_with_snapshot(
    world: &mut EventQueryServiceWorld,
    domain: String,
    count: u32,
    snapshot_seq: u32,
) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let mut pages = Vec::new();
    for seq in 0..count {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    world
        .event_store()
        .add(&domain, DEFAULT_EDITION, root, pages, "")
        .await
        .expect("Failed to add events");

    // Add snapshot
    let snapshot = angzarr::proto::Snapshot {
        sequence: snapshot_seq,
        state: Some(Any {
            type_url: "type.test/State".to_string(),
            value: vec![1, 2, 3],
        }),
        retention: angzarr::proto::SnapshotRetention::RetentionDefault as i32,
    };
    world
        .snapshot_store()
        .put(&domain, DEFAULT_EDITION, root, snapshot)
        .await
        .expect("Failed to add snapshot");

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            event_count: count,
            correlation_id: String::new(),
        },
    );
}

#[given(expr = "aggregates with correlation ID {string}:")]
async fn given_aggregates_with_correlation(
    world: &mut EventQueryServiceWorld,
    step: &Step,
    correlation_id: String,
) {
    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            let domain = row[0].clone();
            let count: u32 = row[1].parse().expect("Invalid event count");
            let root = Uuid::new_v4();

            let mut pages = Vec::new();
            for seq in 0..count {
                pages.push(world.make_event_page(
                    seq,
                    &format!("type.test/Event{}", seq),
                    vec![seq as u8],
                ));
            }

            world
                .event_store()
                .add(&domain, DEFAULT_EDITION, root, pages, &correlation_id)
                .await
                .expect("Failed to add events");

            let key = world.agg_key(&domain, root);
            world.aggregates.insert(
                key,
                AggregateState {
                    domain,
                    root,
                    event_count: count,
                    correlation_id: correlation_id.clone(),
                },
            );
        }
    }
}

#[given(expr = "aggregates in domain {string}:")]
async fn given_aggregates_in_domain(
    world: &mut EventQueryServiceWorld,
    step: &Step,
    domain: String,
) {
    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            let root_name = row[0].clone();
            let count: u32 = row[1].parse().expect("Invalid event count");
            let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

            let mut pages = Vec::new();
            for seq in 0..count {
                pages.push(world.make_event_page(
                    seq,
                    &format!("type.test/Event{}", seq),
                    vec![seq as u8],
                ));
            }

            world
                .event_store()
                .add(&domain, DEFAULT_EDITION, root, pages, "")
                .await
                .expect("Failed to add events");

            let key = world.agg_key(&domain, root);
            world.aggregates.insert(
                key,
                AggregateState {
                    domain: domain.clone(),
                    root,
                    event_count: count,
                    correlation_id: String::new(),
                },
            );
        }
    }
}

#[given(expr = "an aggregate {string} with {int} event")]
async fn given_single_aggregate(world: &mut EventQueryServiceWorld, domain: String, count: u32) {
    given_aggregate_with_events(world, domain, count).await;
}

// ==========================================================================
// When steps - Queries
// ==========================================================================

#[when(expr = "I query the EventBook for domain {string} and the aggregate root")]
async fn when_query_eventbook(world: &mut EventQueryServiceWorld, domain: String) {
    let query = world.make_query(&domain, world.current_root);

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query the EventBook for domain {string} and root {string}")]
async fn when_query_eventbook_by_root(
    world: &mut EventQueryServiceWorld,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let query = world.make_query(&domain, root);

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when("I query the EventBook without a domain or root")]
async fn when_query_without_domain_root(world: &mut EventQueryServiceWorld) {
    let query = Query {
        cover: None,
        selection: None,
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query the EventBook for domain {string} with an invalid root UUID")]
async fn when_query_invalid_uuid(world: &mut EventQueryServiceWorld, domain: String) {
    let query = Query {
        cover: Some(Cover {
            domain,
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid: must be 16 bytes
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query events from sequence {int}")]
async fn when_query_from_sequence(world: &mut EventQueryServiceWorld, from_seq: u32) {
    let query = Query {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: world.current_root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Range(SequenceRange {
            lower: from_seq,
            upper: None,
        })),
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query events from sequence {int} to {int}")]
async fn when_query_range(world: &mut EventQueryServiceWorld, from_seq: u32, to_seq: u32) {
    let query = Query {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: world.current_root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Range(SequenceRange {
            lower: from_seq,
            upper: Some(to_seq),
        })),
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query as of sequence {int}")]
async fn when_query_as_of_sequence(world: &mut EventQueryServiceWorld, seq: u32) {
    let query = Query {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: world.current_root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Temporal(TemporalQuery {
            point_in_time: Some(PointInTime::AsOfSequence(seq)),
        })),
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query as of timestamp {string}")]
async fn when_query_as_of_timestamp(world: &mut EventQueryServiceWorld, timestamp_str: String) {
    let timestamp = parse_iso_timestamp(&timestamp_str);

    let query = Query {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: world.current_root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Temporal(TemporalQuery {
            point_in_time: Some(PointInTime::AsOfTime(timestamp)),
        })),
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when("I query with temporal selection but no point-in-time")]
async fn when_query_temporal_no_point(world: &mut EventQueryServiceWorld) {
    let query = Query {
        cover: Some(Cover {
            domain: world.current_domain.clone(),
            root: Some(ProtoUuid {
                value: world.current_root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Temporal(TemporalQuery {
            point_in_time: None,
        })),
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query by correlation ID {string}")]
async fn when_query_by_correlation(world: &mut EventQueryServiceWorld, correlation_id: String) {
    let query = Query {
        cover: Some(Cover {
            domain: String::new(),
            root: None,
            correlation_id,
            edition: None,
        }),
        selection: None,
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I query by correlation ID {string} without domain or root")]
async fn when_query_correlation_no_domain(
    world: &mut EventQueryServiceWorld,
    correlation_id: String,
) {
    when_query_by_correlation(world, correlation_id).await;
}

#[when("I query the EventBook for the aggregate")]
async fn when_query_current_aggregate(world: &mut EventQueryServiceWorld) {
    let query = world.make_query(&world.current_domain.clone(), world.current_root);

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

// ==========================================================================
// When steps - Streaming
// ==========================================================================

#[when(expr = "I stream events for domain {string} and the aggregate root")]
async fn when_stream_events(world: &mut EventQueryServiceWorld, domain: String) {
    let query = world.make_query(&domain, world.current_root);

    match world.service().get_events(Request::new(query)).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let mut books = Vec::new();
            while let Some(result) = stream.next().await {
                match result {
                    Ok(book) => books.push(book),
                    Err(status) => {
                        world.last_error = Some(status);
                        break;
                    }
                }
            }
            world.last_event_books = books;
            if world.last_error.is_none() {
                world.last_error = None;
            }
        }
        Err(status) => {
            world.last_event_books = Vec::new();
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I stream events for domain {string} and root {string}")]
async fn when_stream_events_by_root(
    world: &mut EventQueryServiceWorld,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let query = world.make_query(&domain, root);

    match world.service().get_events(Request::new(query)).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let mut books = Vec::new();
            while let Some(result) = stream.next().await {
                if let Ok(book) = result {
                    books.push(book);
                }
            }
            world.last_event_books = books;
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_books = Vec::new();
            world.last_error = Some(status);
        }
    }
}

#[when("I stream events without a domain or root")]
async fn when_stream_without_domain_root(world: &mut EventQueryServiceWorld) {
    let query = Query {
        cover: None,
        selection: None,
    };

    match world.service().get_events(Request::new(query)).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let mut books = Vec::new();
            while let Some(result) = stream.next().await {
                if let Ok(book) = result {
                    books.push(book);
                }
            }
            world.last_event_books = books;
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_books = Vec::new();
            world.last_error = Some(status);
        }
    }
}

#[when(expr = "I stream events by correlation ID {string}")]
async fn when_stream_by_correlation(world: &mut EventQueryServiceWorld, correlation_id: String) {
    let query = Query {
        cover: Some(Cover {
            domain: String::new(),
            root: None,
            correlation_id,
            edition: None,
        }),
        selection: None,
    };

    match world.service().get_events(Request::new(query)).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let mut books = Vec::new();
            while let Some(result) = stream.next().await {
                if let Ok(book) = result {
                    books.push(book);
                }
            }
            world.last_event_books = books;
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_books = Vec::new();
            world.last_error = Some(status);
        }
    }
}

// ==========================================================================
// When steps - Root discovery
// ==========================================================================

#[when("I list aggregate roots")]
async fn when_list_roots(world: &mut EventQueryServiceWorld) {
    match world.service().get_aggregate_roots(Request::new(())).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let mut roots = Vec::new();
            while let Some(result) = stream.next().await {
                if let Ok(root) = result {
                    roots.push(root);
                }
            }
            world.last_roots = roots;
            world.last_error = None;
        }
        Err(status) => {
            world.last_roots = Vec::new();
            world.last_error = Some(status);
        }
    }
}

// ==========================================================================
// When steps - Error cases
// ==========================================================================

#[when("I send a malformed query")]
async fn when_send_malformed_query(world: &mut EventQueryServiceWorld) {
    // Empty cover with no correlation_id
    let query = Query {
        cover: Some(Cover {
            domain: String::new(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    match world.service().get_event_book(Request::new(query)).await {
        Ok(response) => {
            world.last_event_book = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_event_book = None;
            world.last_error = Some(status);
        }
    }
}

// ==========================================================================
// Then steps - Event counts
// ==========================================================================

#[then(expr = "I should receive an EventBook with {int} events")]
fn then_receive_eventbook_with_events(world: &mut EventQueryServiceWorld, count: u32) {
    let book = world
        .last_event_book
        .as_ref()
        .expect("No EventBook received");
    assert_eq!(
        book.pages.len() as u32,
        count,
        "Expected {} events, got {}",
        count,
        book.pages.len()
    );
}

#[then("events should be ordered by sequence ascending")]
fn then_events_ordered(world: &mut EventQueryServiceWorld) {
    let book = world
        .last_event_book
        .as_ref()
        .expect("No EventBook received");
    let mut prev_seq: Option<u32> = None;
    for page in &book.pages {
        if let Some(prev) = prev_seq {
            assert!(
                page.sequence > prev,
                "Events not ordered: {} after {}",
                page.sequence,
                prev
            );
        }
        prev_seq = Some(page.sequence);
    }
}

#[then(expr = "the first event should have sequence {int}")]
fn then_first_event_sequence(world: &mut EventQueryServiceWorld, seq: u32) {
    let book = world
        .last_event_book
        .as_ref()
        .expect("No EventBook received");
    let event = book.pages.first().expect("No events found");
    assert_eq!(
        event.sequence, seq,
        "Expected sequence {}, got {}",
        seq, event.sequence
    );
}

#[then(expr = "the last event should have sequence {int}")]
fn then_last_event_sequence(world: &mut EventQueryServiceWorld, seq: u32) {
    let book = world
        .last_event_book
        .as_ref()
        .expect("No EventBook received");
    let event = book.pages.last().expect("No events found");
    assert_eq!(
        event.sequence, seq,
        "Expected sequence {}, got {}",
        seq, event.sequence
    );
}

#[then("the EventBook should not include a snapshot")]
fn then_no_snapshot(world: &mut EventQueryServiceWorld) {
    let book = world
        .last_event_book
        .as_ref()
        .expect("No EventBook received");
    assert!(
        book.snapshot.is_none(),
        "Expected no snapshot, but found one"
    );
}

// ==========================================================================
// Then steps - Errors
// ==========================================================================

#[then("the query should fail with INVALID_ARGUMENT")]
fn then_query_fails_invalid_argument(world: &mut EventQueryServiceWorld) {
    let error = world.last_error.as_ref().expect("Expected error but none");
    assert_eq!(
        error.code(),
        tonic::Code::InvalidArgument,
        "Expected INVALID_ARGUMENT, got {:?}",
        error.code()
    );
}

#[then("the stream should fail with INVALID_ARGUMENT")]
fn then_stream_fails_invalid_argument(world: &mut EventQueryServiceWorld) {
    then_query_fails_invalid_argument(world);
}

#[then("the error message should describe the problem")]
fn then_error_has_message(world: &mut EventQueryServiceWorld) {
    let error = world.last_error.as_ref().expect("Expected error but none");
    assert!(
        !error.message().is_empty(),
        "Expected error message, got empty"
    );
}

// ==========================================================================
// Then steps - Streaming
// ==========================================================================

#[then(expr = "I should receive a stream with {int} EventBook")]
#[then(expr = "I should receive a stream with {int} EventBooks")]
fn then_receive_stream_count(world: &mut EventQueryServiceWorld, count: u32) {
    assert_eq!(
        world.last_event_books.len() as u32,
        count,
        "Expected {} EventBooks, got {}",
        count,
        world.last_event_books.len()
    );
}

#[then(expr = "the EventBook should have {int} events")]
fn then_eventbook_has_events(world: &mut EventQueryServiceWorld, count: u32) {
    let book = world.last_event_books.first().expect("No EventBook found");
    assert_eq!(
        book.pages.len() as u32,
        count,
        "Expected {} events, got {}",
        count,
        book.pages.len()
    );
}

#[then(expr = "the total event count should be {int}")]
fn then_total_event_count(world: &mut EventQueryServiceWorld, count: u32) {
    let total: u32 = world
        .last_event_books
        .iter()
        .map(|b| b.pages.len() as u32)
        .sum();
    assert_eq!(
        total, count,
        "Expected {} total events, got {}",
        count, total
    );
}

// ==========================================================================
// Then steps - Root discovery
// ==========================================================================

#[then(expr = "I should receive a stream with {int} roots")]
fn then_receive_roots(world: &mut EventQueryServiceWorld, count: u32) {
    assert_eq!(
        world.last_roots.len() as u32,
        count,
        "Expected {} roots, got {}",
        count,
        world.last_roots.len()
    );
}

#[then("each root should include domain and UUID")]
fn then_roots_have_domain_uuid(world: &mut EventQueryServiceWorld) {
    for root in &world.last_roots {
        assert!(!root.domain.is_empty(), "Root missing domain: {:?}", root);
        assert!(root.root.is_some(), "Root missing UUID: {:?}", root);
    }
}

// ==========================================================================
// Helper functions
// ==========================================================================

fn parse_iso_timestamp(s: &str) -> Timestamp {
    use chrono::{DateTime, Utc};

    let dt: DateTime<Utc> = s.parse().unwrap_or_else(|_| {
        // Try parsing with T separator
        format!("{}Z", s)
            .parse()
            .expect("Failed to parse timestamp")
    });

    Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}
