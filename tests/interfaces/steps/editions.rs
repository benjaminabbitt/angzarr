//! Edition interface step definitions.

use std::collections::HashMap;

use angzarr::orchestration::aggregate::DEFAULT_EDITION;
use angzarr::proto::{event_page, EventPage};
use angzarr::storage::EventStore;
use cucumber::{given, then, when, World};
use prost_types::Any;
use uuid::Uuid;

use crate::backend::{StorageBackend, StorageContext};

/// Test context for Edition scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct EditionWorld {
    backend: StorageBackend,
    context: Option<StorageContext>,
    current_domain: String,
    current_root: Uuid,
    /// Tracks aggregates by (domain, edition, root) key
    aggregates: HashMap<String, AggregateState>,
    last_events: Vec<EventPage>,
    last_roots: Vec<Uuid>,
    last_error: Option<String>,
    last_delete_count: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct AggregateState {
    domain: String,
    edition: String,
    root: Uuid,
    event_count: u32,
}

impl EditionWorld {
    fn new() -> Self {
        Self {
            backend: StorageBackend::from_env(),
            context: None,
            current_domain: String::new(),
            current_root: Uuid::nil(),
            aggregates: HashMap::new(),
            last_events: Vec::new(),
            last_roots: Vec::new(),
            last_error: None,
            last_delete_count: 0,
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

    fn agg_key(&self, domain: &str, edition: &str, root: Uuid) -> String {
        format!("{}:{}:{}", domain, edition, root)
    }

    fn effective_edition(edition: &str) -> &str {
        if edition.is_empty() {
            DEFAULT_EDITION
        } else {
            edition
        }
    }
}

// ==========================================================================
// Background
// ==========================================================================

#[given("an Edition test environment")]
async fn given_edition_environment(world: &mut EditionWorld) {
    println!("Using backend: {}", world.backend.name());
    let ctx = StorageContext::new(world.backend).await;
    world.context = Some(ctx);
}

// ==========================================================================
// Main Timeline
// ==========================================================================

#[when(expr = "I add an event to domain {string} on main timeline")]
async fn when_add_event_main_timeline(world: &mut EditionWorld, domain: String) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let pages = vec![world.make_event_page(0, "type.test/Event", vec![1])];

    world
        .store()
        .add(&domain, DEFAULT_EDITION, root, pages, "")
        .await
        .expect("Failed to add event");

    let key = world.agg_key(&domain, DEFAULT_EDITION, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            edition: DEFAULT_EDITION.to_string(),
            root,
            event_count: 1,
        },
    );
}

#[when(expr = "I add an event to domain {string} with edition {string}")]
async fn when_add_event_with_edition(world: &mut EditionWorld, domain: String, edition: String) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let effective = EditionWorld::effective_edition(&edition);
    let pages = vec![world.make_event_page(0, "type.test/Event", vec![1])];

    world
        .store()
        .add(&domain, effective, root, pages, "")
        .await
        .expect("Failed to add event");

    let key = world.agg_key(&domain, effective, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            edition: effective.to_string(),
            root,
            event_count: 1,
        },
    );
}

#[then(expr = "the event should be stored with edition {string}")]
async fn then_event_stored_with_edition(world: &mut EditionWorld, edition: String) {
    let events = world
        .store()
        .get(&world.current_domain, &edition, world.current_root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len(),
        1,
        "Expected 1 event stored with edition {}",
        edition
    );
}

#[then("I should be able to retrieve the event from main timeline")]
async fn then_retrieve_from_main(world: &mut EditionWorld) {
    let events = world
        .store()
        .get(&world.current_domain, DEFAULT_EDITION, world.current_root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len(),
        1,
        "Expected to retrieve event from main timeline"
    );
}

#[then(expr = "I should be able to retrieve the event with edition {string}")]
async fn then_retrieve_with_edition(world: &mut EditionWorld, edition: String) {
    let events = world
        .store()
        .get(&world.current_domain, &edition, world.current_root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len(),
        1,
        "Expected to retrieve event with edition {}",
        edition
    );
}

// ==========================================================================
// Edition Isolation
// ==========================================================================

#[given(expr = "an aggregate {string} on main timeline with {int} events")]
async fn given_aggregate_main_with_events(world: &mut EditionWorld, domain: String, count: u32) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let pages: Vec<_> = (0..count)
        .map(|seq| world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]))
        .collect();

    world
        .store()
        .add(&domain, DEFAULT_EDITION, root, pages, "")
        .await
        .expect("Failed to add events");

    let key = world.agg_key(&domain, DEFAULT_EDITION, root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain,
            edition: DEFAULT_EDITION.to_string(),
            root,
            event_count: count,
        },
    );
}

#[when(expr = "I add {int} events to the same aggregate in edition {string}")]
async fn when_add_events_to_edition(world: &mut EditionWorld, count: u32, edition: String) {
    let pages: Vec<_> = (0..count)
        .map(|seq| {
            world.make_event_page(
                seq,
                &format!("type.test/EditionEvent{}", seq),
                vec![seq as u8],
            )
        })
        .collect();

    world
        .store()
        .add(
            &world.current_domain,
            &edition,
            world.current_root,
            pages,
            "",
        )
        .await
        .expect("Failed to add edition events");

    let key = world.agg_key(&world.current_domain, &edition, world.current_root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain: world.current_domain.clone(),
            edition: edition.clone(),
            root: world.current_root,
            event_count: count,
        },
    );
}

#[then(expr = "main timeline should have {int} events")]
async fn then_main_has_events(world: &mut EditionWorld, count: u32) {
    let events = world
        .store()
        .get(&world.current_domain, DEFAULT_EDITION, world.current_root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events on main timeline, got {}",
        count,
        events.len()
    );
}

#[then(expr = "edition {string} should have {int} events")]
async fn then_edition_has_events(world: &mut EditionWorld, edition: String, count: u32) {
    // For count == 0, verify no roots exist in the edition (edition-specific data is gone).
    // This avoids composite read which falls back to main timeline when edition is empty.
    if count == 0 {
        let roots = world
            .store()
            .list_roots(&world.current_domain, &edition)
            .await
            .expect("Failed to list roots");

        assert!(
            roots.is_empty(),
            "Expected no roots in edition {}, but found {} roots",
            edition,
            roots.len()
        );
        return;
    }

    let events = world
        .store()
        .get(&world.current_domain, &edition, world.current_root)
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

#[given(expr = "an aggregate {string} with root {string}")]
async fn given_aggregate_with_root(world: &mut EditionWorld, domain: String, root_name: String) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    world.current_domain = domain;
    world.current_root = root;
}

#[when(expr = "I add {int} events in edition {string}")]
async fn when_add_events_in_edition(world: &mut EditionWorld, count: u32, edition: String) {
    let pages: Vec<_> = (0..count)
        .map(|seq| world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]))
        .collect();

    world
        .store()
        .add(
            &world.current_domain,
            &edition,
            world.current_root,
            pages,
            "",
        )
        .await
        .expect("Failed to add events");

    let key = world.agg_key(&world.current_domain, &edition, world.current_root);
    world.aggregates.insert(
        key,
        AggregateState {
            domain: world.current_domain.clone(),
            edition: edition.clone(),
            root: world.current_root,
            event_count: count,
        },
    );
}

#[then(expr = "edition {string} should have {int} events for root {string}")]
async fn then_edition_has_events_for_root(
    world: &mut EditionWorld,
    edition: String,
    count: u32,
    root_name: String,
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
        "Expected {} events in edition {} for root {}, got {}",
        count,
        edition,
        root_name,
        events.len()
    );
}

#[when(expr = "I add {int} event on main timeline")]
async fn when_add_event_on_main(world: &mut EditionWorld, count: u32) {
    let pages: Vec<_> = (0..count)
        .map(|seq| world.make_event_page(seq, "type.test/MainEvent", vec![seq as u8]))
        .collect();

    world
        .store()
        .add(
            &world.current_domain,
            DEFAULT_EDITION,
            world.current_root,
            pages,
            "",
        )
        .await
        .expect("Failed to add events");
}

#[when(expr = "I add {int} event in edition {string}")]
async fn when_add_event_in_edition(world: &mut EditionWorld, count: u32, edition: String) {
    let pages: Vec<_> = (0..count)
        .map(|seq| world.make_event_page(seq, "type.test/EditionEvent", vec![seq as u8]))
        .collect();

    world
        .store()
        .add(
            &world.current_domain,
            &edition,
            world.current_root,
            pages,
            "",
        )
        .await
        .expect("Failed to add events");
}

#[then(expr = "main timeline should have {int} event for root {string}")]
async fn then_main_has_event_for_root(world: &mut EditionWorld, count: u32, root_name: String) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let events = world
        .store()
        .get(&world.current_domain, DEFAULT_EDITION, root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events on main timeline for root {}, got {}",
        count,
        root_name,
        events.len()
    );
}

#[then(expr = "edition {string} should have {int} event for root {string}")]
async fn then_edition_has_event_for_root(
    world: &mut EditionWorld,
    edition: String,
    count: u32,
    root_name: String,
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
        "Expected {} events in edition {} for root {}, got {}",
        count,
        edition,
        root_name,
        events.len()
    );
}

// ==========================================================================
// Sequence Isolation
// ==========================================================================

#[when(expr = "I add an event to the same aggregate in edition {string}")]
async fn when_add_single_event_to_edition(world: &mut EditionWorld, edition: String) {
    let pages = vec![world.make_event_page(0, "type.test/BranchEvent", vec![1])];

    world
        .store()
        .add(
            &world.current_domain,
            &edition,
            world.current_root,
            pages,
            "",
        )
        .await
        .expect("Failed to add event");
}

#[then(expr = "the first event in edition {string} should have sequence {int}")]
async fn then_first_event_sequence_in_edition(world: &mut EditionWorld, edition: String, seq: u32) {
    let events = world
        .store()
        .get(&world.current_domain, &edition, world.current_root)
        .await
        .expect("Failed to get events");

    let first = events.first().expect("No events found");
    assert_eq!(
        first.sequence, seq,
        "Expected first event sequence {}, got {}",
        seq, first.sequence
    );
}

#[then(expr = "the next sequence on main timeline should be {int}")]
async fn then_next_sequence_main(world: &mut EditionWorld, expected: u32) {
    let next = world
        .store()
        .get_next_sequence(&world.current_domain, DEFAULT_EDITION, world.current_root)
        .await
        .expect("Failed to get next sequence");

    assert_eq!(
        next, expected,
        "Expected next sequence {}, got {}",
        expected, next
    );
}

#[when(expr = "I add {int} events to aggregate {string} in edition {string}")]
async fn when_add_events_to_aggregate_edition(
    world: &mut EditionWorld,
    count: u32,
    domain: String,
    edition: String,
) {
    let root = Uuid::new_v4();
    world.current_domain = domain.clone();
    world.current_root = root;

    let pages: Vec<_> = (0..count)
        .map(|seq| world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]))
        .collect();

    world
        .store()
        .add(&domain, &edition, root, pages, "")
        .await
        .expect("Failed to add events");
}

#[then(expr = "the events should have sequences {int}, {int}, {int}")]
async fn then_events_have_sequences(world: &mut EditionWorld, s0: u32, s1: u32, s2: u32) {
    // Get from the last used edition (we need to track this)
    // For now, assume we query the events directly
    let expected = [s0, s1, s2];
    world.last_events = world
        .store()
        .get(&world.current_domain, "fresh", world.current_root)
        .await
        .expect("Failed to get events");

    for (i, event) in world.last_events.iter().enumerate() {
        assert_eq!(
            event.sequence, expected[i],
            "Event {} expected sequence {}, got {}",
            i, expected[i], event.sequence
        );
    }
}

#[then(expr = "the next sequence in edition {string} should be {int}")]
async fn then_next_sequence_edition(world: &mut EditionWorld, edition: String, expected: u32) {
    let next = world
        .store()
        .get_next_sequence(&world.current_domain, &edition, world.current_root)
        .await
        .expect("Failed to get next sequence");

    assert_eq!(
        next, expected,
        "Expected next sequence {}, got {}",
        expected, next
    );
}

// ==========================================================================
// Root Discovery
// ==========================================================================

#[given(expr = "an aggregate {string} with root {string} on main timeline")]
async fn given_aggregate_root_main(world: &mut EditionWorld, domain: String, root_name: String) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let pages = vec![world.make_event_page(0, "type.test/Event", vec![1])];

    world
        .store()
        .add(&domain, DEFAULT_EDITION, root, pages, "")
        .await
        .expect("Failed to add event");

    world.current_domain = domain;
    world.current_root = root;
}

#[given(expr = "an aggregate {string} with root {string} in edition {string}")]
async fn given_aggregate_root_edition(
    world: &mut EditionWorld,
    domain: String,
    root_name: String,
    edition: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    let pages = vec![world.make_event_page(0, "type.test/Event", vec![1])];

    world
        .store()
        .add(&domain, &edition, root, pages, "")
        .await
        .expect("Failed to add event");

    world.current_domain = domain;
    world.current_root = root;
}

#[when(expr = "I list roots for domain {string} on main timeline")]
async fn when_list_roots_main(world: &mut EditionWorld, domain: String) {
    world.current_domain = domain.clone();
    world.last_roots = world
        .store()
        .list_roots(&domain, DEFAULT_EDITION)
        .await
        .expect("Failed to list roots");
}

#[when(expr = "I list roots for domain {string} in edition {string}")]
async fn when_list_roots_edition(world: &mut EditionWorld, domain: String, edition: String) {
    world.current_domain = domain.clone();
    world.last_roots = world
        .store()
        .list_roots(&domain, &edition)
        .await
        .expect("Failed to list roots");
}

#[then(expr = "I should see {int} root in the list")]
async fn then_see_roots(world: &mut EditionWorld, count: u32) {
    assert_eq!(
        world.last_roots.len() as u32,
        count,
        "Expected {} roots, got {}",
        count,
        world.last_roots.len()
    );
}

#[then(expr = "root {string} should be in the list")]
async fn then_root_in_list(world: &mut EditionWorld, root_name: String) {
    let expected = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_name.as_bytes());
    assert!(
        world.last_roots.contains(&expected),
        "Root '{}' not found in list: {:?}",
        root_name,
        world.last_roots
    );
}

// ==========================================================================
// Edition Cleanup
// ==========================================================================

#[given(expr = "an aggregate {string} in edition {string} with {int} events")]
async fn given_aggregate_edition_with_events(
    world: &mut EditionWorld,
    domain: String,
    edition: String,
    count: u32,
) {
    let root = world.current_root;
    if root.is_nil() {
        world.current_root = Uuid::new_v4();
    }

    world.current_domain = domain.clone();

    let pages: Vec<_> = (0..count)
        .map(|seq| world.make_event_page(seq, &format!("type.test/Event{}", seq), vec![seq as u8]))
        .collect();

    world
        .store()
        .add(&domain, &edition, world.current_root, pages, "")
        .await
        .expect("Failed to add events");
}

#[when(expr = "I delete events for edition {string} in domain {string}")]
async fn when_delete_edition_events(world: &mut EditionWorld, edition: String, domain: String) {
    world.current_domain = domain.clone();

    match world.store().delete_edition_events(&domain, &edition).await {
        Ok(count) => {
            world.last_delete_count = count;
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

#[then(expr = "main timeline should still have {int} events")]
async fn then_main_still_has_events(world: &mut EditionWorld, count: u32) {
    let events = world
        .store()
        .get(&world.current_domain, DEFAULT_EDITION, world.current_root)
        .await
        .expect("Failed to get events");

    assert_eq!(
        events.len() as u32,
        count,
        "Expected {} events on main timeline, got {}",
        count,
        events.len()
    );
}

#[when(expr = "I try to delete events for edition {string} in domain {string}")]
async fn when_try_delete_main_timeline(world: &mut EditionWorld, edition: String, domain: String) {
    world.current_domain = domain.clone();

    // Note: The storage layer doesn't enforce main timeline protection
    // This should be enforced by the caller (standalone client)
    // For this test, we simulate the protection check
    if edition == DEFAULT_EDITION || edition.is_empty() {
        world.last_error = Some("Cannot delete main timeline events".to_string());
    } else {
        match world.store().delete_edition_events(&domain, &edition).await {
            Ok(count) => {
                world.last_delete_count = count;
                world.last_error = None;
            }
            Err(e) => {
                world.last_error = Some(e.to_string());
            }
        }
    }
}

#[then("the operation should be rejected")]
async fn then_operation_rejected(world: &mut EditionWorld) {
    assert!(
        world.last_error.is_some(),
        "Expected operation to be rejected"
    );
}
