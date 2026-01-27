//! EventStore interface tests.
//!
//! These tests verify the contract of the EventStore trait.
//! Each storage implementation should run these tests.

use prost_types::Any;
use uuid::Uuid;

use angzarr::proto::{event_page, EventPage};
use angzarr::storage::EventStore;

/// Create a test event with given sequence and type.
pub fn make_event(seq: u32, event_type: &str) -> EventPage {
    EventPage {
        sequence: Some(event_page::Sequence::Num(seq)),
        created_at: None,
        event: Some(Any {
            type_url: format!("type.example/{}", event_type),
            value: vec![1, 2, 3, seq as u8],
        }),
    }
}

/// Create multiple sequential events.
pub fn make_events(start: u32, count: u32) -> Vec<EventPage> {
    (start..start + count)
        .map(|i| make_event(i, &format!("Event{}", i)))
        .collect()
}

// =============================================================================
// EventStore::add tests
// =============================================================================

pub async fn test_add_single_event<S: EventStore>(store: &S) {
    let domain = "test_add_single";
    let root = Uuid::new_v4();

    store
        .add(domain, root, vec![make_event(0, "Created")], "")
        .await
        .expect("add should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert_eq!(events.len(), 1, "should have 1 event");
}

pub async fn test_add_multiple_events<S: EventStore>(store: &S) {
    let domain = "test_add_multiple";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert_eq!(events.len(), 5, "should have 5 events");
}

pub async fn test_add_empty_events<S: EventStore>(store: &S) {
    let domain = "test_add_empty";
    let root = Uuid::new_v4();

    store
        .add(domain, root, vec![], "")
        .await
        .expect("empty add should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert!(events.is_empty(), "should have no events");
}

pub async fn test_add_sequential_batches<S: EventStore>(store: &S) {
    let domain = "test_add_sequential";
    let root = Uuid::new_v4();

    // First batch: events 0, 1
    store
        .add(domain, root, make_events(0, 2), "")
        .await
        .expect("first batch should succeed");

    // Second batch: events 2, 3, 4
    store
        .add(domain, root, make_events(2, 3), "")
        .await
        .expect("second batch should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert_eq!(events.len(), 5, "should have 5 events total");

    // Verify sequence continuity
    for (i, event) in events.iter().enumerate() {
        if let Some(event_page::Sequence::Num(seq)) = event.sequence {
            assert_eq!(seq, i as u32, "sequence {} should match index", i);
        }
    }
}

pub async fn test_add_sequence_conflict<S: EventStore>(store: &S) {
    let domain = "test_seq_conflict";
    let root = Uuid::new_v4();

    // Add events 0, 1, 2
    store
        .add(domain, root, make_events(0, 3), "")
        .await
        .expect("first add should succeed");

    // Try to add with sequence lower than current (rewinding) - should fail
    // This tests the common contract: can't rewrite history
    let result = store
        .add(domain, root, vec![make_event(1, "Rewind")], "")
        .await;
    assert!(result.is_err(), "sequence lower than current should fail");
}

pub async fn test_add_duplicate_sequence<S: EventStore>(store: &S) {
    let domain = "test_dup_seq";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 3), "")
        .await
        .expect("first add should succeed");

    // Try to add at sequence 0 again
    let result = store
        .add(domain, root, vec![make_event(0, "Dup")], "")
        .await;
    assert!(result.is_err(), "duplicate sequence should fail");
}

// =============================================================================
// EventStore::get tests
// =============================================================================

pub async fn test_get_all_events<S: EventStore>(store: &S) {
    let domain = "test_get_all";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 10), "")
        .await
        .expect("add should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert_eq!(events.len(), 10, "should return all events");

    // Verify order
    for (i, event) in events.iter().enumerate() {
        if let Some(event_page::Sequence::Num(seq)) = event.sequence {
            assert_eq!(seq, i as u32, "events should be ordered");
        }
    }
}

pub async fn test_get_empty_aggregate<S: EventStore>(store: &S) {
    let domain = "test_get_empty";
    let root = Uuid::new_v4();

    let events = store.get(domain, root).await.expect("get should succeed");
    assert!(events.is_empty(), "non-existent aggregate should be empty");
}

pub async fn test_get_preserves_event_data<S: EventStore>(store: &S) {
    let domain = "test_preserve_data";
    let root = Uuid::new_v4();

    let original = EventPage {
        sequence: Some(event_page::Sequence::Num(0)),
        created_at: None,
        event: Some(Any {
            type_url: "type.example/TestEvent".to_string(),
            value: vec![10, 20, 30, 40, 50, 100, 200],
        }),
    };

    store
        .add(domain, root, vec![original], "")
        .await
        .expect("add should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert_eq!(events.len(), 1);

    let event = &events[0];
    let payload = event.event.as_ref().expect("event should have payload");
    assert_eq!(payload.type_url, "type.example/TestEvent");
    assert_eq!(payload.value, vec![10, 20, 30, 40, 50, 100, 200]);
}

// =============================================================================
// EventStore::get_from tests
// =============================================================================

pub async fn test_get_from_zero<S: EventStore>(store: &S) {
    let domain = "test_get_from_zero";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from(domain, root, 0)
        .await
        .expect("get_from should succeed");
    assert_eq!(events.len(), 5, "get_from(0) should return all events");
}

pub async fn test_get_from_middle<S: EventStore>(store: &S) {
    let domain = "test_get_from_mid";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 10), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from(domain, root, 5)
        .await
        .expect("get_from should succeed");
    assert_eq!(events.len(), 5, "should return events 5-9");

    if let Some(event_page::Sequence::Num(seq)) = events[0].sequence {
        assert_eq!(seq, 5, "first event should be sequence 5");
    }
}

pub async fn test_get_from_end<S: EventStore>(store: &S) {
    let domain = "test_get_from_end";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from(domain, root, 100)
        .await
        .expect("get_from should succeed");
    assert!(events.is_empty(), "get_from beyond end should be empty");
}

pub async fn test_get_from_last<S: EventStore>(store: &S) {
    let domain = "test_get_from_last";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from(domain, root, 4)
        .await
        .expect("get_from should succeed");
    assert_eq!(events.len(), 1, "should return last event only");
}

// =============================================================================
// EventStore::get_from_to tests
// =============================================================================

pub async fn test_get_from_to_full_range<S: EventStore>(store: &S) {
    let domain = "test_range_full";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from_to(domain, root, 0, 5)
        .await
        .expect("get_from_to should succeed");
    assert_eq!(events.len(), 5, "should return all events");
}

pub async fn test_get_from_to_partial<S: EventStore>(store: &S) {
    let domain = "test_range_partial";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 10), "")
        .await
        .expect("add should succeed");

    // Get events 3, 4, 5, 6 (exclusive end at 7)
    let events = store
        .get_from_to(domain, root, 3, 7)
        .await
        .expect("get_from_to should succeed");
    assert_eq!(events.len(), 4, "should return 4 events");

    if let Some(event_page::Sequence::Num(seq)) = events[0].sequence {
        assert_eq!(seq, 3, "first should be 3");
    }
    if let Some(event_page::Sequence::Num(seq)) = events[3].sequence {
        assert_eq!(seq, 6, "last should be 6");
    }
}

pub async fn test_get_from_to_single<S: EventStore>(store: &S) {
    let domain = "test_range_single";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from_to(domain, root, 2, 3)
        .await
        .expect("get_from_to should succeed");
    assert_eq!(events.len(), 1, "should return single event");

    if let Some(event_page::Sequence::Num(seq)) = events[0].sequence {
        assert_eq!(seq, 2);
    }
}

pub async fn test_get_from_to_empty<S: EventStore>(store: &S) {
    let domain = "test_range_empty";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 5), "")
        .await
        .expect("add should succeed");

    let events = store
        .get_from_to(domain, root, 100, 200)
        .await
        .expect("get_from_to should succeed");
    assert!(events.is_empty(), "out of range should be empty");
}

// =============================================================================
// EventStore::list_roots tests
// =============================================================================

pub async fn test_list_roots_single<S: EventStore>(store: &S) {
    let domain = "test_roots_single";
    let root = Uuid::new_v4();

    store
        .add(domain, root, vec![make_event(0, "E")], "")
        .await
        .expect("add should succeed");

    let roots = store
        .list_roots(domain)
        .await
        .expect("list_roots should succeed");
    assert_eq!(roots.len(), 1);
    assert!(roots.contains(&root));
}

pub async fn test_list_roots_multiple<S: EventStore>(store: &S) {
    let domain = "test_roots_multi";
    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();
    let root3 = Uuid::new_v4();

    store
        .add(domain, root1, vec![make_event(0, "E1")], "")
        .await
        .unwrap();
    store
        .add(domain, root2, vec![make_event(0, "E2")], "")
        .await
        .unwrap();
    store
        .add(domain, root3, vec![make_event(0, "E3")], "")
        .await
        .unwrap();

    let roots = store
        .list_roots(domain)
        .await
        .expect("list_roots should succeed");
    assert_eq!(roots.len(), 3);
    assert!(roots.contains(&root1));
    assert!(roots.contains(&root2));
    assert!(roots.contains(&root3));
}

pub async fn test_list_roots_empty_domain<S: EventStore>(store: &S) {
    let roots = store
        .list_roots("nonexistent_domain_xyz")
        .await
        .expect("list_roots should succeed");
    assert!(roots.is_empty());
}

pub async fn test_list_roots_domain_isolation<S: EventStore>(store: &S) {
    let domain1 = "test_roots_d1";
    let domain2 = "test_roots_d2";
    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();

    store
        .add(domain1, root1, vec![make_event(0, "E1")], "")
        .await
        .unwrap();
    store
        .add(domain2, root2, vec![make_event(0, "E2")], "")
        .await
        .unwrap();

    let roots1 = store.list_roots(domain1).await.unwrap();
    let roots2 = store.list_roots(domain2).await.unwrap();

    assert_eq!(roots1.len(), 1);
    assert_eq!(roots2.len(), 1);
    assert!(roots1.contains(&root1));
    assert!(!roots1.contains(&root2));
    assert!(roots2.contains(&root2));
    assert!(!roots2.contains(&root1));
}

// =============================================================================
// EventStore::list_domains tests
// =============================================================================

pub async fn test_list_domains_contains<S: EventStore>(store: &S) {
    let domain = format!("test_domain_{}", Uuid::new_v4());
    let root = Uuid::new_v4();

    store
        .add(&domain, root, vec![make_event(0, "E")], "")
        .await
        .expect("add should succeed");

    let domains = store
        .list_domains()
        .await
        .expect("list_domains should succeed");
    assert!(domains.contains(&domain), "should contain new domain");
}

pub async fn test_list_domains_multiple<S: EventStore>(store: &S) {
    let domains_to_add: Vec<String> = (0..3)
        .map(|i| format!("test_multi_domain_{}_{}", i, Uuid::new_v4()))
        .collect();

    for domain in &domains_to_add {
        store
            .add(domain, Uuid::new_v4(), vec![make_event(0, "E")], "")
            .await
            .unwrap();
    }

    let domains = store
        .list_domains()
        .await
        .expect("list_domains should succeed");
    for domain in &domains_to_add {
        assert!(domains.contains(domain), "should contain {}", domain);
    }
}

// =============================================================================
// EventStore::get_next_sequence tests
// =============================================================================

pub async fn test_get_next_sequence_empty<S: EventStore>(store: &S) {
    let next = store
        .get_next_sequence("nonexistent_seq", Uuid::new_v4())
        .await
        .expect("get_next_sequence should succeed");
    assert_eq!(next, 0, "empty aggregate should have next seq 0");
}

pub async fn test_get_next_sequence_after_events<S: EventStore>(store: &S) {
    let domain = "test_next_seq";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 7), "")
        .await
        .expect("add should succeed");

    let next = store
        .get_next_sequence(domain, root)
        .await
        .expect("get_next_sequence should succeed");
    assert_eq!(next, 7, "next should be 7 after adding 0-6");
}

pub async fn test_get_next_sequence_increments<S: EventStore>(store: &S) {
    let domain = "test_seq_inc";
    let root = Uuid::new_v4();

    assert_eq!(store.get_next_sequence(domain, root).await.unwrap(), 0);

    store
        .add(domain, root, vec![make_event(0, "E0")], "")
        .await
        .unwrap();
    assert_eq!(store.get_next_sequence(domain, root).await.unwrap(), 1);

    store
        .add(domain, root, make_events(1, 3), "")
        .await
        .unwrap();
    assert_eq!(store.get_next_sequence(domain, root).await.unwrap(), 4);
}

// =============================================================================
// Integration tests
// =============================================================================

pub async fn test_aggregate_isolation<S: EventStore>(store: &S) {
    let domain = "test_isolation";
    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();

    store
        .add(domain, root1, make_events(0, 3), "")
        .await
        .unwrap();
    store
        .add(domain, root2, make_events(0, 5), "")
        .await
        .unwrap();

    let events1 = store.get(domain, root1).await.unwrap();
    let events2 = store.get(domain, root2).await.unwrap();

    assert_eq!(events1.len(), 3);
    assert_eq!(events2.len(), 5);

    assert_eq!(store.get_next_sequence(domain, root1).await.unwrap(), 3);
    assert_eq!(store.get_next_sequence(domain, root2).await.unwrap(), 5);
}

pub async fn test_large_batch<S: EventStore>(store: &S) {
    let domain = "test_large";
    let root = Uuid::new_v4();

    store
        .add(domain, root, make_events(0, 100), "")
        .await
        .expect("large batch should succeed");

    let events = store.get(domain, root).await.expect("get should succeed");
    assert_eq!(events.len(), 100);

    for (i, event) in events.iter().enumerate() {
        if let Some(event_page::Sequence::Num(seq)) = event.sequence {
            assert_eq!(seq, i as u32);
        }
    }
}

// =============================================================================
// Test runner macro
// =============================================================================

/// Run all EventStore interface tests against a store implementation.
#[macro_export]
macro_rules! run_event_store_tests {
    ($store:expr) => {
        use $crate::storage::event_store_tests::*;

        // add tests
        test_add_single_event($store).await;
        println!("  test_add_single_event: PASSED");

        test_add_multiple_events($store).await;
        println!("  test_add_multiple_events: PASSED");

        test_add_empty_events($store).await;
        println!("  test_add_empty_events: PASSED");

        test_add_sequential_batches($store).await;
        println!("  test_add_sequential_batches: PASSED");

        test_add_sequence_conflict($store).await;
        println!("  test_add_sequence_conflict: PASSED");

        test_add_duplicate_sequence($store).await;
        println!("  test_add_duplicate_sequence: PASSED");

        // get tests
        test_get_all_events($store).await;
        println!("  test_get_all_events: PASSED");

        test_get_empty_aggregate($store).await;
        println!("  test_get_empty_aggregate: PASSED");

        test_get_preserves_event_data($store).await;
        println!("  test_get_preserves_event_data: PASSED");

        // get_from tests
        test_get_from_zero($store).await;
        println!("  test_get_from_zero: PASSED");

        test_get_from_middle($store).await;
        println!("  test_get_from_middle: PASSED");

        test_get_from_end($store).await;
        println!("  test_get_from_end: PASSED");

        test_get_from_last($store).await;
        println!("  test_get_from_last: PASSED");

        // get_from_to tests
        test_get_from_to_full_range($store).await;
        println!("  test_get_from_to_full_range: PASSED");

        test_get_from_to_partial($store).await;
        println!("  test_get_from_to_partial: PASSED");

        test_get_from_to_single($store).await;
        println!("  test_get_from_to_single: PASSED");

        test_get_from_to_empty($store).await;
        println!("  test_get_from_to_empty: PASSED");

        // list_roots tests
        test_list_roots_single($store).await;
        println!("  test_list_roots_single: PASSED");

        test_list_roots_multiple($store).await;
        println!("  test_list_roots_multiple: PASSED");

        test_list_roots_empty_domain($store).await;
        println!("  test_list_roots_empty_domain: PASSED");

        test_list_roots_domain_isolation($store).await;
        println!("  test_list_roots_domain_isolation: PASSED");

        // list_domains tests
        test_list_domains_contains($store).await;
        println!("  test_list_domains_contains: PASSED");

        test_list_domains_multiple($store).await;
        println!("  test_list_domains_multiple: PASSED");

        // get_next_sequence tests
        test_get_next_sequence_empty($store).await;
        println!("  test_get_next_sequence_empty: PASSED");

        test_get_next_sequence_after_events($store).await;
        println!("  test_get_next_sequence_after_events: PASSED");

        test_get_next_sequence_increments($store).await;
        println!("  test_get_next_sequence_increments: PASSED");

        // integration tests
        test_aggregate_isolation($store).await;
        println!("  test_aggregate_isolation: PASSED");

        test_large_batch($store).await;
        println!("  test_large_batch: PASSED");
    };
}
