//! EventStore interface step definitions.

use std::collections::HashMap;

use angzarr::proto::{event_page, EventBook, EventPage};
use angzarr::storage::EventStore;
use cucumber::{given, then, when, World};
use prost_types::Any;
use uuid::Uuid;

use crate::backend::{StorageBackend, StorageContext};

/// Test context for EventStore scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct EventStoreWorld {
    backend: StorageBackend,
    context: Option<StorageContext>,
    current_domain: String,
    current_root: Uuid,
    current_edition: String,
    aggregates: HashMap<String, AggregateState>,
    last_events: Vec<EventPage>,
    last_event_books: Vec<EventBook>,
    last_next_seq: u32,
    last_domains: Vec<String>,
    last_roots: Vec<Uuid>,
    last_error: Option<String>,
    stored_timestamp: Option<prost_types::Timestamp>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct AggregateState {
    domain: String,
    root: Uuid,
    edition: String,
    event_count: u32,
}

impl EventStoreWorld {
    fn new() -> Self {
        Self {
            backend: StorageBackend::from_env(),
            context: None,
            current_domain: String::new(),
            current_root: Uuid::nil(),
            current_edition: "test".to_string(),
            aggregates: HashMap::new(),
            last_events: Vec::new(),
            last_event_books: Vec::new(),
            last_next_seq: 0,
            last_domains: Vec::new(),
            last_roots: Vec::new(),
            last_error: None,
            stored_timestamp: None,
        }
    }

    fn store(&self) -> &dyn EventStore {
        self.context
            .as_ref()
            .expect("Storage context not initialized")
            .event_store
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

    fn agg_key(&self, domain: &str, root: Uuid) -> String {
        format!("{}:{}", domain, root)
    }

    fn agg_key_with_edition(&self, domain: &str, edition: &str, root: Uuid) -> String {
        format!("{}:{}:{}", domain, edition, root)
    }
}

// --- Background ---

#[given("an EventStore backend")]
async fn given_event_store_backend(world: &mut EventStoreWorld) {
    println!("Using backend: {}", world.backend.name());
    let ctx = StorageContext::new(world.backend).await;
    world.context = Some(ctx);
}

// --- Given steps ---

#[given(expr = "an aggregate {string} with no events")]
async fn given_aggregate_no_events(world: &mut EventStoreWorld, domain: String) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition: "test".to_string(),
            event_count: 0,
        },
    );
}

#[given(expr = "an aggregate {string} with {int} event")]
#[given(expr = "an aggregate {string} with {int} events")]
async fn given_aggregate_with_events(world: &mut EventStoreWorld, domain: String, count: u32) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    // Add events to storage
    let mut pages = Vec::new();
    for seq in 0..count {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        world
            .store()
            .add(&domain, "test", root, pages, "test-correlation")
            .await
            .expect("Failed to add events");
    }

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition: "test".to_string(),
            event_count: count,
        },
    );
}

#[given(expr = "{int} aggregates in domain {string} each with {int} event")]
async fn given_multiple_aggregates(
    world: &mut EventStoreWorld,
    count: u32,
    domain: String,
    event_count: u32,
) {
    world.current_domain = domain.clone();

    for _ in 0..count {
        let root = Uuid::new_v4();

        let mut pages = Vec::new();
        for seq in 0..event_count {
            pages.push(world.make_event_page(
                seq,
                &format!("type.test/Event{}", seq),
                vec![seq as u8],
            ));
        }

        if !pages.is_empty() {
            world
                .store()
                .add(&domain, "test", root, pages, "test-correlation")
                .await
                .expect("Failed to add events");
        }

        let key = world.agg_key(&domain, root);
        world.aggregates.insert(
            key,
            AggregateState {
                domain: domain.clone(),
                root,
                edition: "test".to_string(),
                event_count,
            },
        );
    }
}

#[given(expr = "an aggregate {string} with root {string} and {int} events")]
async fn given_aggregate_with_root(
    world: &mut EventStoreWorld,
    domain: String,
    root_name: String,
    count: u32,
) {
    // Create deterministic UUID from name
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;

    let mut pages = Vec::new();
    for seq in 0..count {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        world
            .store()
            .add(&domain, "test", root, pages, "test-correlation")
            .await
            .expect("Failed to add events");
    }

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition: "test".to_string(),
            event_count: count,
        },
    );
}

// --- When steps ---

#[when(expr = "I add {int} event to the aggregate")]
#[when(expr = "I add {int} events to the aggregate")]
async fn when_add_events(world: &mut EventStoreWorld, count: u32) {
    let key = world.agg_key(&world.current_domain, world.current_root);
    let state = world
        .aggregates
        .get(&key)
        .expect("Aggregate not found")
        .clone();

    let start_seq = state.event_count;
    let mut pages = Vec::new();

    for i in 0..count {
        let seq = start_seq + i;
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        match world
            .store()
            .add(
                &world.current_domain,
                "test",
                world.current_root,
                pages,
                "test-correlation",
            )
            .await
        {
            Ok(_) => {
                let state = world.aggregates.get_mut(&key).unwrap();
                state.event_count += count;
            }
            Err(e) => {
                world.last_error = Some(e.to_string());
            }
        }
    }
}

#[when(expr = "I try to add an event with sequence {int}")]
async fn when_try_add_event_at_sequence(world: &mut EventStoreWorld, seq: u32) {
    let pages = vec![world.make_event_page(seq, "type.test/ConflictEvent", vec![seq as u8])];

    match world
        .store()
        .add(
            &world.current_domain,
            "test",
            world.current_root,
            pages,
            "test-correlation",
        )
        .await
    {
        Ok(_) => {
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[when("I get all events from the aggregate")]
async fn when_get_all_events(world: &mut EventStoreWorld) {
    world.last_events = world
        .store()
        .get(&world.current_domain, "test", world.current_root)
        .await
        .expect("Failed to get events");
}

#[when(expr = "I add an event with type {string} and payload {string}")]
async fn when_add_event_with_type_payload(
    world: &mut EventStoreWorld,
    type_name: String,
    payload: String,
) {
    let key = world.agg_key(&world.current_domain, world.current_root);
    let state = world
        .aggregates
        .get(&key)
        .expect("Aggregate not found")
        .clone();

    let pages = vec![world.make_event_page(
        state.event_count,
        &format!("type.test/{}", type_name),
        payload.into_bytes(),
    )];

    world
        .store()
        .add(
            &world.current_domain,
            "test",
            world.current_root,
            pages,
            "test-correlation",
        )
        .await
        .expect("Failed to add event");

    let state = world.aggregates.get_mut(&key).unwrap();
    state.event_count += 1;
}

#[when(expr = "I get events from sequence {int}")]
async fn when_get_events_from_sequence(world: &mut EventStoreWorld, from_seq: u32) {
    world.last_events = world
        .store()
        .get_from(&world.current_domain, "test", world.current_root, from_seq)
        .await
        .expect("Failed to get events");
}

#[when(expr = "I get events from sequence {int} to {int}")]
async fn when_get_events_in_range(world: &mut EventStoreWorld, from_seq: u32, to_seq: u32) {
    world.last_events = world
        .store()
        .get_from_to(
            &world.current_domain,
            "test",
            world.current_root,
            from_seq,
            to_seq,
        )
        .await
        .expect("Failed to get events");
}

#[when(expr = "I list roots for domain {string}")]
async fn when_list_roots(world: &mut EventStoreWorld, domain: String) {
    world.last_roots = world
        .store()
        .list_roots(&domain, "test")
        .await
        .expect("Failed to list roots");
}

#[when("I list all domains")]
async fn when_list_domains(world: &mut EventStoreWorld) {
    world.last_domains = world
        .store()
        .list_domains()
        .await
        .expect("Failed to list domains");
}

#[when("I get the next sequence for the aggregate")]
async fn when_get_next_sequence(world: &mut EventStoreWorld) {
    world.last_next_seq = world
        .store()
        .get_next_sequence(&world.current_domain, "test", world.current_root)
        .await
        .expect("Failed to get next sequence");
}

#[when(expr = "I get events for root {string} in domain {string}")]
async fn when_get_events_for_root(world: &mut EventStoreWorld, root_name: String, domain: String) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    world.last_events = world
        .store()
        .get(&domain, "test", root)
        .await
        .expect("Failed to get events");
}

// --- Then steps ---

#[then(expr = "the aggregate should have {int} event")]
#[then(expr = "the aggregate should have {int} events")]
async fn then_aggregate_has_events(world: &mut EventStoreWorld, count: u32) {
    let events = world
        .store()
        .get(&world.current_domain, "test", world.current_root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events, got {}",
        count,
        events.len()
    );

    // Store events for subsequent "first event" / "last event" checks
    world.last_events = events;
}

#[then(expr = "the first event should have sequence {int}")]
fn then_first_event_sequence(world: &mut EventStoreWorld, seq: u32) {
    let event = world.last_events.first().expect("No events found");
    assert_eq!(
        event.sequence, seq,
        "Expected sequence {}, got {}",
        seq, event.sequence
    );
}

#[then(expr = "the last event should have sequence {int}")]
fn then_last_event_sequence(world: &mut EventStoreWorld, seq: u32) {
    let event = world.last_events.last().expect("No events found");
    assert_eq!(
        event.sequence, seq,
        "Expected sequence {}, got {}",
        seq, event.sequence
    );
}

#[then("events should have consecutive sequences starting from 0")]
fn then_consecutive_sequences_from_zero(world: &mut EventStoreWorld) {
    for (i, event) in world.last_events.iter().enumerate() {
        assert_eq!(
            event.sequence, i as u32,
            "Expected sequence {}, got {}",
            i, event.sequence
        );
    }
}

#[then("the operation should fail with a sequence conflict")]
fn then_sequence_conflict(world: &mut EventStoreWorld) {
    assert!(
        world.last_error.is_some(),
        "Expected error but operation succeeded"
    );
    let error = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error.contains("conflict") || error.contains("sequence") || error.contains("duplicate"),
        "Expected sequence conflict error, got: {}",
        error
    );
}

#[then(expr = "I should receive {int} event")]
#[then(expr = "I should receive {int} events")]
fn then_receive_events(world: &mut EventStoreWorld, count: u32) {
    assert_eq!(
        world.last_events.len() as u32,
        count,
        "Expected {} events, got {}",
        count,
        world.last_events.len()
    );
}

#[then("events should be ordered by sequence ascending")]
fn then_events_ordered(world: &mut EventStoreWorld) {
    let mut prev_seq: Option<u32> = None;
    for event in &world.last_events {
        let seq = event.sequence;
        if let Some(prev) = prev_seq {
            assert!(seq > prev, "Events not ordered: {} after {}", seq, prev);
        }
        prev_seq = Some(seq);
    }
}

#[then(expr = "the first event should have type {string}")]
fn then_first_event_type(world: &mut EventStoreWorld, expected_type: String) {
    let event = world.last_events.first().expect("No events found");
    let type_url = match &event.payload {
        Some(event_page::Payload::Event(e)) => e.type_url.clone(),
        _ => panic!("No event data"),
    };
    assert!(
        type_url.contains(&expected_type),
        "Expected type containing '{}', got '{}'",
        expected_type,
        type_url
    );
}

#[then(expr = "the first event should have payload {string}")]
fn then_first_event_payload(world: &mut EventStoreWorld, expected_payload: String) {
    let event = world.last_events.first().expect("No events found");
    let payload = match &event.payload {
        Some(event_page::Payload::Event(e)) => &e.value,
        _ => panic!("No event data"),
    };
    let payload_str = String::from_utf8_lossy(payload);
    assert_eq!(
        payload_str, expected_payload,
        "Expected payload '{}', got '{}'",
        expected_payload, payload_str
    );
}

#[then(expr = "I should see {int} root in the list")]
#[then(expr = "I should see {int} roots in the list")]
fn then_see_roots(world: &mut EventStoreWorld, count: u32) {
    assert_eq!(
        world.last_roots.len() as u32,
        count,
        "Expected {} roots, got {}",
        count,
        world.last_roots.len()
    );
}

#[then(expr = "the root should not appear in domain {string}")]
async fn then_root_not_in_domain(world: &mut EventStoreWorld, domain: String) {
    // Get roots from the target domain we're checking against
    let target_roots = world
        .store()
        .list_roots(&domain, "test")
        .await
        .expect("Failed to list roots");

    // Check that none of our previously listed roots appear in the target domain
    // (last_roots was set by the "When I list roots" step)
    for root in &world.last_roots {
        let found = target_roots.iter().any(|r| r == root);
        assert!(
            !found,
            "Root {} unexpectedly found in domain {}",
            root, domain
        );
    }
}

#[then(expr = "the domain list should contain {string}")]
fn then_domain_list_contains(world: &mut EventStoreWorld, domain: String) {
    assert!(
        world.last_domains.contains(&domain),
        "Domain '{}' not found in list: {:?}",
        domain,
        world.last_domains
    );
}

#[then(expr = "the next sequence should be {int}")]
fn then_next_sequence(world: &mut EventStoreWorld, expected: u32) {
    assert_eq!(
        world.last_next_seq, expected,
        "Expected next sequence {}, got {}",
        expected, world.last_next_seq
    );
}

// ==========================================================================
// Edition Support Steps
// ==========================================================================

#[given(expr = "an aggregate {string} with root {string} in edition {string}")]
async fn given_aggregate_with_root_edition(
    world: &mut EventStoreWorld,
    domain: String,
    root_name: String,
    edition: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;
    world.current_edition = edition.clone();

    // Add an initial event so the aggregate exists in storage
    let page = world.make_event_page(0, "type.test/InitEvent", vec![0]);
    world
        .store()
        .add(&domain, &edition, root, vec![page], "test-correlation")
        .await
        .expect("Failed to add initial event");

    let key = world.agg_key_with_edition(&domain, &edition, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition,
            event_count: 1,
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} in edition {string} with {int} events")]
async fn given_aggregate_with_root_edition_with_events(
    world: &mut EventStoreWorld,
    domain: String,
    root_name: String,
    edition: String,
    count: u32,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;
    world.current_edition = edition.clone();

    let mut pages = Vec::new();
    for seq in 0..count {
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        world
            .store()
            .add(&domain, &edition, root, pages, "test-correlation")
            .await
            .expect("Failed to add events");
    }

    let key = world.agg_key_with_edition(&domain, &edition, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition,
            event_count: count,
        },
    );
}

#[when(expr = "I add {int} events to {string} in edition {string}")]
async fn when_add_events_to_root_edition(
    world: &mut EventStoreWorld,
    count: u32,
    root_name: String,
    edition: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let key = world.agg_key_with_edition(&world.current_domain, &edition, root);

    let state = world
        .aggregates
        .get(&key)
        .cloned()
        .unwrap_or(AggregateState {
            domain: world.current_domain.clone(),
            root,
            edition: edition.clone(),
            event_count: 0,
        });

    let start_seq = state.event_count;
    let mut pages = Vec::new();

    for i in 0..count {
        let seq = start_seq + i;
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    if !pages.is_empty() {
        world
            .store()
            .add(
                &world.current_domain,
                &edition,
                root,
                pages,
                "test-correlation",
            )
            .await
            .expect("Failed to add events");
    }

    let entry = world.aggregates.entry(key).or_insert(AggregateState {
        domain: world.current_domain.clone(),
        root,
        edition: edition.clone(),
        event_count: 0,
    });
    entry.event_count += count;
}

#[then(expr = "{string} in edition {string} should have {int} events")]
async fn then_root_in_edition_has_events(
    world: &mut EventStoreWorld,
    root_name: String,
    edition: String,
    count: u32,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let events = world
        .store()
        .get(&world.current_domain, &edition, root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events in edition {}, got {}",
        count,
        edition,
        events.len()
    );
}

#[then(expr = "the next sequence for {string} in edition {string} should be {int}")]
async fn then_next_seq_for_root_edition(
    world: &mut EventStoreWorld,
    root_name: String,
    edition: String,
    expected: u32,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let next_seq = world
        .store()
        .get_next_sequence(&world.current_domain, &edition, root)
        .await
        .expect("Failed to get next sequence");

    assert_eq!(
        next_seq, expected,
        "Expected next sequence {} in edition {}, got {}",
        expected, edition, next_seq
    );
}

#[when(expr = "I list roots for domain {string} in edition {string}")]
async fn when_list_roots_in_edition(world: &mut EventStoreWorld, domain: String, edition: String) {
    world.last_roots = world
        .store()
        .list_roots(&domain, &edition)
        .await
        .expect("Failed to list roots");
}

// ==========================================================================
// Correlation ID Steps
// ==========================================================================

#[given(expr = "an aggregate {string} with root {string} and correlation ID {string}")]
async fn given_aggregate_with_correlation(
    world: &mut EventStoreWorld,
    domain: String,
    root_name: String,
    correlation_id: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;

    // Add one event with the correlation ID
    let page = world.make_event_page(0, "type.test/Event0", vec![0]);
    world
        .store()
        .add(&domain, "test", root, vec![page], &correlation_id)
        .await
        .expect("Failed to add event");

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition: "test".to_string(),
            event_count: 1,
        },
    );
}

#[when(expr = "I query events by correlation ID {string}")]
async fn when_query_by_correlation(world: &mut EventStoreWorld, correlation_id: String) {
    world.last_event_books = world
        .store()
        .get_by_correlation(&correlation_id)
        .await
        .expect("Failed to query by correlation ID");
}

#[then(expr = "I should receive events from {int} aggregate")]
#[then(expr = "I should receive events from {int} aggregates")]
fn then_receive_from_aggregates(world: &mut EventStoreWorld, count: u32) {
    assert_eq!(
        world.last_event_books.len() as u32,
        count,
        "Expected {} aggregates, got {}",
        count,
        world.last_event_books.len()
    );
}

#[given(expr = "an aggregate {string} with root {string} and no events")]
async fn given_aggregate_with_root_no_events(
    world: &mut EventStoreWorld,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;

    let key = world.agg_key(&domain, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition: "test".to_string(),
            event_count: 0,
        },
    );
}

#[when(expr = "I add an event with correlation ID {string}")]
async fn when_add_event_with_correlation(world: &mut EventStoreWorld, correlation_id: String) {
    let key = world.agg_key(&world.current_domain, world.current_root);
    let state = world
        .aggregates
        .get(&key)
        .expect("Aggregate not found")
        .clone();

    let page = world.make_event_page(state.event_count, "type.test/Event", vec![0]);
    world
        .store()
        .add(
            &world.current_domain,
            "test",
            world.current_root,
            vec![page],
            &correlation_id,
        )
        .await
        .expect("Failed to add event");

    let state = world.aggregates.get_mut(&key).unwrap();
    state.event_count += 1;
}

// ==========================================================================
// Timestamp Preservation Steps
// ==========================================================================

#[when("I add an event with a known timestamp")]
async fn when_add_event_with_known_timestamp(world: &mut EventStoreWorld) {
    let key = world.agg_key(&world.current_domain, world.current_root);
    let state = world
        .aggregates
        .get(&key)
        .expect("Aggregate not found")
        .clone();

    let timestamp = prost_types::Timestamp::from(std::time::SystemTime::now());
    world.stored_timestamp = Some(timestamp);

    let page = EventPage {
        sequence: state.event_count,
        created_at: Some(timestamp),
        payload: Some(event_page::Payload::Event(Any {
            type_url: "type.test/TimestampEvent".to_string(),
            value: vec![0],
        })),
    };

    world
        .store()
        .add(
            &world.current_domain,
            "test",
            world.current_root,
            vec![page],
            "test-correlation",
        )
        .await
        .expect("Failed to add event");

    let state = world.aggregates.get_mut(&key).unwrap();
    state.event_count += 1;
}

#[then("the first event timestamp should match the stored timestamp")]
fn then_timestamp_matches(world: &mut EventStoreWorld) {
    let event = world.last_events.first().expect("No events found");
    let event_ts = event.created_at.as_ref().expect("No timestamp on event");
    let stored_ts = world
        .stored_timestamp
        .as_ref()
        .expect("No stored timestamp");

    assert_eq!(
        event_ts.seconds, stored_ts.seconds,
        "Timestamps should match (seconds)"
    );
    // Note: Some stores may not preserve nanosecond precision
}
