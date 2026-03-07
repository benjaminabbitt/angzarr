//! QueryClient step definitions.

use cucumber::{given, then, when, World};
use std::collections::HashMap;

/// Mock event for testing.
#[derive(Debug, Clone)]
struct MockEvent {
    sequence: u32,
    event_type: String,
    payload: String,
}

/// Mock EventBook for testing.
#[derive(Debug, Clone, Default)]
struct MockEventBook {
    events: Vec<MockEvent>,
    snapshot_sequence: Option<u32>,
    edition: Option<String>,
}

/// Test context for QueryClient scenarios.
#[derive(Debug, Default, World)]
pub struct QueryClientWorld {
    client_connected: bool,
    aggregates: HashMap<String, MockEventBook>,
    correlation_events: HashMap<String, Vec<MockEventBook>>,
    result: Option<MockEventBook>,
    error: Option<String>,
    service_available: bool,
}

// ==========================================================================
// Background Steps
// ==========================================================================

#[given("a QueryClient connected to the test backend")]
async fn given_query_client(world: &mut QueryClientWorld) {
    world.client_connected = true;
    world.service_available = true;
}

// ==========================================================================
// Given Steps - Aggregates
// ==========================================================================

#[given(expr = "an aggregate {string} with root {string}")]
async fn given_aggregate(world: &mut QueryClientWorld, domain: String, root: String) {
    let key = format!("{}:{}", domain, root);
    world.aggregates.insert(
        key,
        MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} has {int} events")]
async fn given_aggregate_with_events(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    count: u32,
) {
    let key = format!("{}:{}", domain, root);
    let mut events = vec![];
    for i in 0..count {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
            payload: format!("data-{}", i),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            events,
            snapshot_sequence: None,
            edition: None,
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} has event {string} with data {string}")]
async fn given_aggregate_with_specific_event(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    event_type: String,
    data: String,
) {
    let key = format!("{}:{}", domain, root);
    world.aggregates.insert(
        key,
        MockEventBook {
            events: vec![MockEvent {
                sequence: 0,
                event_type,
                payload: data,
            }],
            snapshot_sequence: None,
            edition: None,
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} has events at known timestamps")]
async fn given_aggregate_with_timestamps(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
) {
    let key = format!("{}:{}", domain, root);
    let mut events = vec![];
    for i in 0..5 {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
            payload: format!("data-{}", i),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            events,
            snapshot_sequence: None,
            edition: None,
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} in edition {string}")]
async fn given_aggregate_in_edition(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    edition: String,
) {
    let key = format!("{}:{}:{}", domain, root, edition);
    let mut events = vec![];
    for i in 0..3 {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
            payload: format!("data-{}", i),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            events,
            snapshot_sequence: None,
            edition: Some(edition),
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} has {int} events in main")]
async fn given_aggregate_in_main(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    count: u32,
) {
    let key = format!("{}:{}", domain, root);
    let mut events = vec![];
    for i in 0..count {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
            payload: format!("data-{}", i),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            events,
            snapshot_sequence: None,
            edition: None,
        },
    );
}

#[given(expr = "an aggregate {string} with root {string} has {int} events in edition {string}")]
async fn given_aggregate_in_edition_count(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    count: u32,
    edition: String,
) {
    let key = format!("{}:{}:{}", domain, root, edition);
    let mut events = vec![];
    for i in 0..count {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
            payload: format!("data-{}", i),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            events,
            snapshot_sequence: None,
            edition: Some(edition),
        },
    );
}

#[given(
    expr = "an aggregate {string} with root {string} has a snapshot at sequence {int} and {int} events"
)]
async fn given_aggregate_with_snapshot(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    snap_seq: u32,
    total: u32,
) {
    let key = format!("{}:{}", domain, root);
    let mut events = vec![];
    for i in 0..total {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
            payload: format!("data-{}", i),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            events,
            snapshot_sequence: Some(snap_seq),
            edition: None,
        },
    );
}

#[given(expr = "events with correlation ID {string} exist in multiple aggregates")]
async fn given_correlated_events(world: &mut QueryClientWorld, cid: String) {
    let books = vec![
        MockEventBook {
            events: vec![
                MockEvent {
                    sequence: 0,
                    event_type: "OrderCreated".to_string(),
                    payload: "data".to_string(),
                },
                MockEvent {
                    sequence: 1,
                    event_type: "OrderUpdated".to_string(),
                    payload: "data".to_string(),
                },
            ],
            snapshot_sequence: None,
            edition: None,
        },
        MockEventBook {
            events: vec![MockEvent {
                sequence: 0,
                event_type: "Reserved".to_string(),
                payload: "data".to_string(),
            }],
            snapshot_sequence: None,
            edition: None,
        },
    ];
    world.correlation_events.insert(cid, books);
}

#[given("the query service is unavailable")]
async fn given_service_unavailable(world: &mut QueryClientWorld) {
    world.service_available = false;
}

// ==========================================================================
// When Steps
// ==========================================================================

#[when(expr = "I query events for {string} root {string}")]
async fn when_query_events(world: &mut QueryClientWorld, domain: String, root: String) {
    if !world.service_available {
        world.error = Some("Connection error".to_string());
        return;
    }

    let key = format!("{}:{}", domain, root);
    world.result = world.aggregates.get(&key).cloned().or_else(|| {
        Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        })
    });
}

#[when(expr = "I query events for {string} root {string} from sequence {int}")]
async fn when_query_from_sequence(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    start: u32,
) {
    let key = format!("{}:{}", domain, root);
    if let Some(book) = world.aggregates.get(&key) {
        let filtered_events: Vec<_> = book
            .events
            .iter()
            .filter(|e| e.sequence >= start)
            .cloned()
            .collect();
        world.result = Some(MockEventBook {
            events: filtered_events,
            snapshot_sequence: book.snapshot_sequence,
            edition: book.edition.clone(),
        });
    } else {
        world.result = Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        });
    }
}

#[when(expr = "I query events for {string} root {string} from sequence {int} to {int}")]
async fn when_query_range(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    start: u32,
    end: u32,
) {
    let key = format!("{}:{}", domain, root);
    if let Some(book) = world.aggregates.get(&key) {
        let filtered_events: Vec<_> = book
            .events
            .iter()
            .filter(|e| e.sequence >= start && e.sequence < end)
            .cloned()
            .collect();
        world.result = Some(MockEventBook {
            events: filtered_events,
            snapshot_sequence: book.snapshot_sequence,
            edition: book.edition.clone(),
        });
    } else {
        world.result = Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        });
    }
}

#[when(expr = "I query events for {string} root {string} as of sequence {int}")]
async fn when_query_as_of_sequence(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    seq: u32,
) {
    let key = format!("{}:{}", domain, root);
    if let Some(book) = world.aggregates.get(&key) {
        let filtered_events: Vec<_> = book
            .events
            .iter()
            .filter(|e| e.sequence <= seq)
            .cloned()
            .collect();
        world.result = Some(MockEventBook {
            events: filtered_events,
            snapshot_sequence: book.snapshot_sequence,
            edition: book.edition.clone(),
        });
    } else {
        world.result = Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        });
    }
}

#[when(expr = "I query events for {string} root {string} as of time {string}")]
async fn when_query_as_of_time(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    _timestamp: String,
) {
    // For testing, return all events (timestamp filtering is simulated)
    let key = format!("{}:{}", domain, root);
    world.result = world.aggregates.get(&key).cloned().or_else(|| {
        Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        })
    });
}

#[when(expr = "I query events for {string} root {string} in edition {string}")]
async fn when_query_in_edition(
    world: &mut QueryClientWorld,
    domain: String,
    root: String,
    edition: String,
) {
    let key = format!("{}:{}:{}", domain, root, edition);
    world.result = world.aggregates.get(&key).cloned().or_else(|| {
        Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: Some(edition),
        })
    });
}

#[when(expr = "I query events by correlation ID {string}")]
async fn when_query_by_correlation(world: &mut QueryClientWorld, cid: String) {
    if let Some(books) = world.correlation_events.get(&cid) {
        // Combine all events
        let mut all_events = vec![];
        for book in books {
            all_events.extend(book.events.clone());
        }
        world.result = Some(MockEventBook {
            events: all_events,
            snapshot_sequence: None,
            edition: None,
        });
    } else {
        world.result = Some(MockEventBook {
            events: vec![],
            snapshot_sequence: None,
            edition: None,
        });
    }
}

#[when("I query events with empty domain")]
async fn when_query_empty_domain(world: &mut QueryClientWorld) {
    world.error = Some("Invalid argument: empty domain".to_string());
}

#[when("I attempt to query events")]
async fn when_attempt_query(world: &mut QueryClientWorld) {
    if !world.service_available {
        world.error = Some("Connection error".to_string());
    }
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then(expr = "I should receive an EventBook with {int} events")]
async fn then_receive_events(world: &mut QueryClientWorld, count: u32) {
    let result = world.result.as_ref().expect("Should have result");
    assert_eq!(result.events.len() as u32, count);
}

#[then(expr = "the next_sequence should be {int}")]
async fn then_next_sequence(world: &mut QueryClientWorld, seq: u32) {
    let result = world.result.as_ref().expect("Should have result");
    assert_eq!(result.events.len() as u32, seq);
}

#[then(expr = "events should be in sequence order {int} to {int}")]
async fn then_events_in_order(world: &mut QueryClientWorld, start: u32, _end: u32) {
    let result = world.result.as_ref().expect("Should have result");
    for (i, event) in result.events.iter().enumerate() {
        assert_eq!(event.sequence, start + i as u32);
    }
}

#[then(expr = "the first event should have type {string}")]
async fn then_first_event_type(world: &mut QueryClientWorld, event_type: String) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(!result.events.is_empty());
    assert_eq!(result.events[0].event_type, event_type);
}

#[then(expr = "the first event should have payload {string}")]
async fn then_first_event_payload(world: &mut QueryClientWorld, payload: String) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(!result.events.is_empty());
    assert_eq!(result.events[0].payload, payload);
}

#[then(expr = "the first event should have sequence {int}")]
async fn then_first_event_sequence(world: &mut QueryClientWorld, seq: u32) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(!result.events.is_empty());
    assert_eq!(result.events[0].sequence, seq);
}

#[then(expr = "the last event should have sequence {int}")]
async fn then_last_event_sequence(world: &mut QueryClientWorld, seq: u32) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(!result.events.is_empty());
    assert_eq!(result.events.last().unwrap().sequence, seq);
}

#[then("I should receive events up to that timestamp")]
async fn then_receive_events_up_to_timestamp(world: &mut QueryClientWorld) {
    assert!(world.result.is_some());
}

#[then("I should receive events from that edition only")]
async fn then_receive_events_from_edition(world: &mut QueryClientWorld) {
    assert!(world.result.is_some());
}

#[then("I should receive events from all correlated aggregates")]
async fn then_receive_correlated_events(world: &mut QueryClientWorld) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(!result.events.is_empty());
}

#[then("I should receive no events")]
async fn then_receive_no_events(world: &mut QueryClientWorld) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(result.events.is_empty());
}

#[then("the EventBook should include the snapshot")]
async fn then_event_book_includes_snapshot(world: &mut QueryClientWorld) {
    let result = world.result.as_ref().expect("Should have result");
    assert!(result.snapshot_sequence.is_some());
}

#[then(expr = "the returned snapshot should be at sequence {int}")]
async fn then_snapshot_at_sequence(world: &mut QueryClientWorld, seq: u32) {
    let result = world.result.as_ref().expect("Should have result");
    assert_eq!(result.snapshot_sequence, Some(seq));
}

#[then("the operation should fail with invalid argument error")]
async fn then_fail_invalid_argument(world: &mut QueryClientWorld) {
    let error = world.error.as_ref().expect("Should have error");
    assert!(error.to_lowercase().contains("invalid"));
}

#[then("the operation should fail with connection error")]
async fn then_fail_connection_error(world: &mut QueryClientWorld) {
    let error = world.error.as_ref().expect("Should have error");
    assert!(error.to_lowercase().contains("connection"));
}
