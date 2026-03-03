//! Event Idempotency step definitions.

use std::collections::HashMap;

use angzarr::proto::{event_page, EventPage};
use angzarr::proto_ext::EventPageExt;
use angzarr::storage::{AddOutcome, EventStore};
use cucumber::{given, then, when, World};
use prost_types::Any;
use uuid::Uuid;

use crate::backend::{StorageBackend, StorageContext};

/// Test context for Event Idempotency scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct IdempotencyWorld {
    backend: StorageBackend,
    context: Option<StorageContext>,
    current_domain: String,
    current_edition: String,
    current_root: Uuid,
    aggregates: HashMap<String, AggregateState>,
    last_events: Vec<EventPage>,
    last_outcome: Option<AddOutcome>,
    last_error: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct AggregateState {
    domain: String,
    edition: String,
    root: Uuid,
    event_count: u32,
}

impl IdempotencyWorld {
    fn new() -> Self {
        Self {
            backend: StorageBackend::from_env(),
            context: None,
            current_domain: String::new(),
            current_edition: "test".to_string(),
            current_root: Uuid::nil(),
            aggregates: HashMap::new(),
            last_events: Vec::new(),
            last_outcome: None,
            last_error: None,
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
            sequence_type: Some(event_page::SequenceType::Sequence(seq)),
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

// ==========================================================================
// Background
// ==========================================================================

#[given("an EventStore backend")]
async fn given_event_store_backend(world: &mut IdempotencyWorld) {
    println!("Using backend: {}", world.backend.name());
    let ctx = StorageContext::new(world.backend).await;
    world.context = Some(ctx);
}

// ==========================================================================
// Given steps
// ==========================================================================

#[given(expr = "an aggregate {string} with no events")]
async fn given_aggregate_no_events(world: &mut IdempotencyWorld, domain: String) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;
    world.current_edition = "test".to_string();

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

#[given(expr = "an aggregate {string} with root {string} and no events")]
async fn given_aggregate_with_root_no_events(
    world: &mut IdempotencyWorld,
    domain: String,
    root_name: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;
    world.current_edition = "test".to_string();

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

#[given(expr = "an aggregate {string} with root {string} in edition {string}")]
async fn given_aggregate_with_root_edition(
    world: &mut IdempotencyWorld,
    domain: String,
    root_name: String,
    edition: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain.clone();
    world.current_root = root;
    world.current_edition = edition.clone();

    let key = world.agg_key_with_edition(&domain, &edition, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            root,
            edition,
            event_count: 0,
        },
    );
}

// ==========================================================================
// When steps
// ==========================================================================

#[when(expr = "I add {int} event with external_id {string}")]
#[when(expr = "I add {int} events with external_id {string}")]
async fn when_add_events_with_external_id(
    world: &mut IdempotencyWorld,
    count: u32,
    external_id: String,
) {
    let key = world.agg_key(&world.current_domain, world.current_root);
    let state = world
        .aggregates
        .get(&key)
        .cloned()
        .unwrap_or(AggregateState {
            domain: world.current_domain.clone(),
            root: world.current_root,
            edition: world.current_edition.clone(),
            event_count: 0,
        });

    let start_seq = state.event_count;
    let mut pages = Vec::new();

    for i in 0..count {
        let seq = start_seq + i;
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    // Use empty string external_id as None
    let ext_id = if external_id.is_empty() {
        None
    } else {
        Some(external_id.as_str())
    };

    match world
        .store()
        .add(
            &world.current_domain,
            &world.current_edition,
            world.current_root,
            pages,
            "test-correlation",
            ext_id,
        )
        .await
    {
        Ok(outcome) => {
            // Only update event count if this was a new addition (not duplicate)
            if matches!(outcome, AddOutcome::Added { .. }) {
                let entry = world.aggregates.entry(key).or_insert(AggregateState {
                    domain: world.current_domain.clone(),
                    root: world.current_root,
                    edition: world.current_edition.clone(),
                    event_count: 0,
                });
                entry.event_count += count;
            }
            world.last_outcome = Some(outcome);
            world.last_error = None;
        }
        Err(e) => {
            world.last_outcome = None;
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(expr = "I add {int} event without external_id")]
#[when(expr = "I add {int} events without external_id")]
async fn when_add_events_without_external_id(world: &mut IdempotencyWorld, count: u32) {
    let key = world.agg_key(&world.current_domain, world.current_root);
    let state = world
        .aggregates
        .get(&key)
        .cloned()
        .unwrap_or(AggregateState {
            domain: world.current_domain.clone(),
            root: world.current_root,
            edition: world.current_edition.clone(),
            event_count: 0,
        });

    let start_seq = state.event_count;
    let mut pages = Vec::new();

    for i in 0..count {
        let seq = start_seq + i;
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    match world
        .store()
        .add(
            &world.current_domain,
            &world.current_edition,
            world.current_root,
            pages,
            "test-correlation",
            None,
        )
        .await
    {
        Ok(outcome) => {
            // Events without external_id are always added
            let entry = world.aggregates.entry(key).or_insert(AggregateState {
                domain: world.current_domain.clone(),
                root: world.current_root,
                edition: world.current_edition.clone(),
                event_count: 0,
            });
            entry.event_count += count;
            world.last_outcome = Some(outcome);
            world.last_error = None;
        }
        Err(e) => {
            world.last_outcome = None;
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(expr = "I add {int} event to {string} with external_id {string}")]
async fn when_add_event_to_root_with_external_id(
    world: &mut IdempotencyWorld,
    count: u32,
    root_name: String,
    external_id: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let key = world.agg_key(&world.current_domain, root);
    let state = world
        .aggregates
        .get(&key)
        .cloned()
        .unwrap_or(AggregateState {
            domain: world.current_domain.clone(),
            root,
            edition: world.current_edition.clone(),
            event_count: 0,
        });

    let start_seq = state.event_count;
    let mut pages = Vec::new();

    for i in 0..count {
        let seq = start_seq + i;
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    let ext_id = if external_id.is_empty() {
        None
    } else {
        Some(external_id.as_str())
    };

    match world
        .store()
        .add(
            &world.current_domain,
            &world.current_edition,
            root,
            pages,
            "test-correlation",
            ext_id,
        )
        .await
    {
        Ok(outcome) => {
            if matches!(outcome, AddOutcome::Added { .. }) {
                let entry = world.aggregates.entry(key).or_insert(AggregateState {
                    domain: world.current_domain.clone(),
                    root,
                    edition: world.current_edition.clone(),
                    event_count: 0,
                });
                entry.event_count += count;
            }
            world.last_outcome = Some(outcome);
            world.last_error = None;
        }
        Err(e) => {
            world.last_outcome = None;
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(expr = "I add {int} event to {string} in domain {string} with external_id {string}")]
async fn when_add_event_to_root_in_domain_with_external_id(
    world: &mut IdempotencyWorld,
    count: u32,
    root_name: String,
    domain: String,
    external_id: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let key = world.agg_key(&domain, root);
    let state = world
        .aggregates
        .get(&key)
        .cloned()
        .unwrap_or(AggregateState {
            domain: domain.clone(),
            root,
            edition: world.current_edition.clone(),
            event_count: 0,
        });

    let start_seq = state.event_count;
    let mut pages = Vec::new();

    for i in 0..count {
        let seq = start_seq + i;
        pages.push(world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]));
    }

    let ext_id = if external_id.is_empty() {
        None
    } else {
        Some(external_id.as_str())
    };

    match world
        .store()
        .add(
            &domain,
            &world.current_edition,
            root,
            pages,
            "test-correlation",
            ext_id,
        )
        .await
    {
        Ok(outcome) => {
            if matches!(outcome, AddOutcome::Added { .. }) {
                let entry = world.aggregates.entry(key).or_insert(AggregateState {
                    domain: domain.clone(),
                    root,
                    edition: world.current_edition.clone(),
                    event_count: 0,
                });
                entry.event_count += count;
            }
            world.last_outcome = Some(outcome);
            world.last_error = None;
        }
        Err(e) => {
            world.last_outcome = None;
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(expr = "I add {int} event to {string} in edition {string} with external_id {string}")]
async fn when_add_event_to_root_in_edition_with_external_id(
    world: &mut IdempotencyWorld,
    count: u32,
    root_name: String,
    edition: String,
    external_id: String,
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

    let ext_id = if external_id.is_empty() {
        None
    } else {
        Some(external_id.as_str())
    };

    match world
        .store()
        .add(
            &world.current_domain,
            &edition,
            root,
            pages,
            "test-correlation",
            ext_id,
        )
        .await
    {
        Ok(outcome) => {
            if matches!(outcome, AddOutcome::Added { .. }) {
                let entry = world.aggregates.entry(key).or_insert(AggregateState {
                    domain: world.current_domain.clone(),
                    root,
                    edition: edition.clone(),
                    event_count: 0,
                });
                entry.event_count += count;
            }
            world.last_outcome = Some(outcome);
            world.last_error = None;
        }
        Err(e) => {
            world.last_outcome = None;
            world.last_error = Some(e.to_string());
        }
    }
}

#[when(expr = "I add {int} events with external_id {string} concurrently {int} times")]
async fn when_add_events_concurrently(
    world: &mut IdempotencyWorld,
    count: u32,
    external_id: String,
    concurrent_count: u32,
) {
    use futures::future::join_all;

    let key = world.agg_key(&world.current_domain, world.current_root);

    // Create pages that would be added
    let mut pages = Vec::new();
    for i in 0..count {
        pages.push(world.make_event_page(i, &format!("type.test/Event{}", i), vec![i as u8]));
    }

    let store = world.store();
    let domain = world.current_domain.clone();
    let edition = world.current_edition.clone();
    let root = world.current_root;

    // Run concurrent adds
    let futures: Vec<_> = (0..concurrent_count)
        .map(|_| {
            let pages_clone = pages.clone();
            let domain_clone = domain.clone();
            let edition_clone = edition.clone();
            let external_id_clone = external_id.clone();
            async move {
                store
                    .add(
                        &domain_clone,
                        &edition_clone,
                        root,
                        pages_clone,
                        "test-correlation",
                        Some(&external_id_clone),
                    )
                    .await
            }
        })
        .collect();

    let results = join_all(futures).await;

    // Count how many succeeded with "Added" vs "Duplicate"
    let mut added_count = 0;
    let mut duplicate_count = 0;
    let mut last_outcome = None;

    for result in results {
        match result {
            Ok(outcome) => {
                match &outcome {
                    AddOutcome::Added { .. } => added_count += 1,
                    AddOutcome::Duplicate { .. } => duplicate_count += 1,
                }
                last_outcome = Some(outcome);
            }
            Err(e) => {
                world.last_error = Some(e.to_string());
            }
        }
    }

    // Only one should have been Added, the rest should be Duplicates
    assert_eq!(
        added_count, 1,
        "Expected exactly 1 Added outcome, got {}",
        added_count
    );
    assert_eq!(
        duplicate_count,
        concurrent_count - 1,
        "Expected {} Duplicate outcomes, got {}",
        concurrent_count - 1,
        duplicate_count
    );

    // Update state
    let entry = world.aggregates.entry(key).or_insert(AggregateState {
        domain: world.current_domain.clone(),
        root: world.current_root,
        edition: world.current_edition.clone(),
        event_count: 0,
    });
    entry.event_count += count;

    world.last_outcome = last_outcome;
}

// ==========================================================================
// Then steps
// ==========================================================================

#[then(expr = "the aggregate should have {int} event")]
#[then(expr = "the aggregate should have {int} events")]
async fn then_aggregate_has_events(world: &mut IdempotencyWorld, count: u32) {
    let events = world
        .store()
        .get(
            &world.current_domain,
            &world.current_edition,
            world.current_root,
        )
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events, got {}",
        count,
        events.len()
    );

    world.last_events = events;
}

#[then(expr = "the add outcome should be {string}")]
fn then_add_outcome_is(world: &mut IdempotencyWorld, expected: String) {
    let outcome = world
        .last_outcome
        .as_ref()
        .expect("No add outcome recorded");

    let actual = match outcome {
        AddOutcome::Added { .. } => "added",
        AddOutcome::Duplicate { .. } => "duplicate",
    };

    assert_eq!(
        actual, expected,
        "Expected outcome '{}', got '{}'",
        expected, actual
    );
}

#[then(expr = "the outcome should report first_sequence {int}")]
fn then_outcome_first_sequence(world: &mut IdempotencyWorld, expected: u32) {
    let outcome = world
        .last_outcome
        .as_ref()
        .expect("No add outcome recorded");

    let first_seq = match outcome {
        AddOutcome::Added { first_sequence, .. } => *first_sequence,
        AddOutcome::Duplicate { first_sequence, .. } => *first_sequence,
    };

    assert_eq!(
        first_seq, expected,
        "Expected first_sequence {}, got {}",
        expected, first_seq
    );
}

#[then(expr = "the outcome should report last_sequence {int}")]
fn then_outcome_last_sequence(world: &mut IdempotencyWorld, expected: u32) {
    let outcome = world
        .last_outcome
        .as_ref()
        .expect("No add outcome recorded");

    let last_seq = match outcome {
        AddOutcome::Added { last_sequence, .. } => *last_sequence,
        AddOutcome::Duplicate { last_sequence, .. } => *last_sequence,
    };

    assert_eq!(
        last_seq, expected,
        "Expected last_sequence {}, got {}",
        expected, last_seq
    );
}

#[then("events should have consecutive sequences starting from 0")]
fn then_consecutive_sequences_from_zero(world: &mut IdempotencyWorld) {
    for (i, event) in world.last_events.iter().enumerate() {
        assert_eq!(
            event.sequence_num(),
            i as u32,
            "Expected sequence {}, got {}",
            i,
            event.sequence_num()
        );
    }
}

#[then(expr = "{string} in domain {string} should have {int} event")]
#[then(expr = "{string} in domain {string} should have {int} events")]
async fn then_root_in_domain_has_events(
    world: &mut IdempotencyWorld,
    root_name: String,
    domain: String,
    count: u32,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());

    let events = world
        .store()
        .get(&domain, &world.current_edition, root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events for {} in domain {}, got {}",
        count,
        root_name,
        domain,
        events.len()
    );
}

#[then(expr = "{string} in edition {string} should have {int} event")]
#[then(expr = "{string} in edition {string} should have {int} events")]
async fn then_root_in_edition_has_events(
    world: &mut IdempotencyWorld,
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
        "Expected {} events for {} in edition {}, got {}",
        count,
        root_name,
        edition,
        events.len()
    );
}

#[then(expr = "the first event should have sequence {int}")]
fn then_first_event_sequence(world: &mut IdempotencyWorld, seq: u32) {
    let event = world.last_events.first().expect("No events found");
    assert_eq!(
        event.sequence_num(),
        seq,
        "Expected sequence {}, got {}",
        seq,
        event.sequence_num()
    );
}

#[then(expr = "the last event should have sequence {int}")]
fn then_last_event_sequence(world: &mut IdempotencyWorld, seq: u32) {
    let event = world.last_events.last().expect("No events found");
    assert_eq!(
        event.sequence_num(),
        seq,
        "Expected sequence {}, got {}",
        seq,
        event.sequence_num()
    );
}
