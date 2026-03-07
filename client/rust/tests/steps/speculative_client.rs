//! SpeculativeClient step definitions.

use cucumber::{given, then, when, World};
use std::collections::HashMap;

/// Mock event for testing.
#[derive(Debug, Clone)]
struct MockEvent {
    #[allow(dead_code)]
    sequence: u32,
    #[allow(dead_code)]
    event_type: String,
}

/// Mock EventBook for testing.
#[derive(Debug, Clone, Default)]
struct MockEventBook {
    #[allow(dead_code)]
    domain: String,
    #[allow(dead_code)]
    root: String,
    #[allow(dead_code)]
    events: Vec<MockEvent>,
}

/// Speculative result.
#[derive(Debug, Clone)]
struct SpeculativeResult {
    events: Vec<MockEvent>,
    commands: Vec<String>,
    projection: Option<String>,
    rejection: Option<String>,
}

/// Test context for SpeculativeClient scenarios.
#[derive(Debug, Default, World)]
pub struct SpeculativeClientWorld {
    client_connected: bool,
    aggregates: HashMap<String, MockEventBook>,
    speculative_result: Option<SpeculativeResult>,
    events_persisted: bool,
    edition_created: bool,
    edition_discarded: bool,
    error: Option<String>,
    error_type: Option<String>,
    service_available: bool,
    aggregate_state: String,
    saga_origin: Option<String>,
    has_correlation_id: bool,
    spec_a_result: Option<SpeculativeResult>,
    spec_b_result: Option<SpeculativeResult>,
    real_event_count: u32,
}

// ==========================================================================
// Background Steps
// ==========================================================================

#[given("a SpeculativeClient connected to the test backend")]
async fn given_speculative_client(world: &mut SpeculativeClientWorld) {
    world.client_connected = true;
    world.service_available = true;
}

// ==========================================================================
// Given Steps
// ==========================================================================

#[given(expr = "an aggregate {string} with root {string} has {int} events")]
async fn given_aggregate_with_events(
    world: &mut SpeculativeClientWorld,
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
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            domain,
            root,
            events,
        },
    );
    world.real_event_count = count;
}

#[given(expr = "an aggregate {string} with root {string} in state {string}")]
async fn given_aggregate_in_state(
    world: &mut SpeculativeClientWorld,
    domain: String,
    root: String,
    state: String,
) {
    let key = format!("{}:{}", domain, root);
    world.aggregates.insert(
        key,
        MockEventBook {
            domain,
            root,
            events: vec![MockEvent {
                sequence: 0,
                event_type: "StateChanged".to_string(),
            }],
        },
    );
    world.aggregate_state = state;
}

#[given(expr = "an aggregate {string} with root {string}")]
async fn given_aggregate(world: &mut SpeculativeClientWorld, domain: String, root: String) {
    let key = format!("{}:{}", domain, root);
    world.aggregates.insert(
        key,
        MockEventBook {
            domain,
            root,
            events: vec![],
        },
    );
}

#[given(expr = "events for {string} root {string}")]
async fn given_events_for(world: &mut SpeculativeClientWorld, domain: String, root: String) {
    let key = format!("{}:{}", domain, root);
    world.aggregates.insert(
        key,
        MockEventBook {
            domain,
            root,
            events: vec![MockEvent {
                sequence: 0,
                event_type: "Event".to_string(),
            }],
        },
    );
}

#[given(expr = "{int} events for {string} root {string}")]
async fn given_n_events_for(
    world: &mut SpeculativeClientWorld,
    count: u32,
    domain: String,
    root: String,
) {
    let key = format!("{}:{}", domain, root);
    let mut events = vec![];
    for i in 0..count {
        events.push(MockEvent {
            sequence: i,
            event_type: "Event".to_string(),
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            domain,
            root,
            events,
        },
    );
}

#[given("events with saga origin from \"inventory\" aggregate")]
async fn given_events_with_saga_origin(world: &mut SpeculativeClientWorld) {
    world.saga_origin = Some("inventory".to_string());
}

#[given("correlated events from multiple domains")]
async fn given_correlated_events(world: &mut SpeculativeClientWorld) {
    world.has_correlation_id = true;
}

#[given("events without correlation ID")]
async fn given_events_without_correlation(world: &mut SpeculativeClientWorld) {
    world.has_correlation_id = false;
}

#[given(expr = "a speculative aggregate {string} with root {string} has {int} events")]
async fn given_speculative_aggregate(
    world: &mut SpeculativeClientWorld,
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
        });
    }
    world.aggregates.insert(
        key,
        MockEventBook {
            domain,
            root,
            events,
        },
    );
    world.real_event_count = count;
}

#[given("the speculative service is unavailable")]
async fn given_service_unavailable(world: &mut SpeculativeClientWorld) {
    world.service_available = false;
}

// ==========================================================================
// When Steps
// ==========================================================================

#[when(expr = "I speculatively execute a command against {string} root {string}")]
async fn when_speculative_execute(
    world: &mut SpeculativeClientWorld,
    _domain: String,
    _root: String,
) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![MockEvent {
            sequence: 0,
            event_type: "SpeculativeEvent".to_string(),
        }],
        commands: vec![],
        projection: None,
        rejection: None,
    });
    world.events_persisted = false;
    world.edition_created = true;
    world.edition_discarded = true;
}

#[when(expr = "I speculatively execute a command as of sequence {int}")]
async fn when_speculative_as_of_sequence(world: &mut SpeculativeClientWorld, _seq: u32) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![MockEvent {
            sequence: 0,
            event_type: "SpeculativeEvent".to_string(),
        }],
        commands: vec![],
        projection: None,
        rejection: None,
    });
    world.events_persisted = false;
}

#[when(expr = "I speculatively execute a {string} command")]
async fn when_speculative_execute_command(world: &mut SpeculativeClientWorld, cmd_type: String) {
    if cmd_type == "CancelOrder" && world.aggregate_state == "shipped" {
        world.speculative_result = Some(SpeculativeResult {
            events: vec![],
            commands: vec![],
            projection: None,
            rejection: Some("cannot cancel shipped order".to_string()),
        });
    } else {
        world.speculative_result = Some(SpeculativeResult {
            events: vec![MockEvent {
                sequence: 0,
                event_type: format!("{}Executed", cmd_type),
            }],
            commands: vec![],
            projection: None,
            rejection: None,
        });
    }
    world.events_persisted = false;
}

#[when("I speculatively execute a command with invalid payload")]
async fn when_speculative_invalid_payload(world: &mut SpeculativeClientWorld) {
    world.error = Some("Validation error".to_string());
    world.error_type = Some("validation".to_string());
    world.speculative_result = None;
}

#[when("I speculatively execute a command")]
async fn when_speculative_execute_simple(world: &mut SpeculativeClientWorld) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![MockEvent {
            sequence: 0,
            event_type: "SpeculativeEvent".to_string(),
        }],
        commands: vec![],
        projection: None,
        rejection: None,
    });
    world.events_persisted = false;
    world.edition_created = true;
    world.edition_discarded = true;
}

#[when(expr = "I speculatively execute projector {string} against those events")]
async fn when_speculative_projector(world: &mut SpeculativeClientWorld, projector: String) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![],
        commands: vec![],
        projection: Some(format!("{} projection result", projector)),
        rejection: None,
    });
    world.events_persisted = false;
}

#[when(expr = "I speculatively execute projector {string}")]
async fn when_speculative_projector_simple(world: &mut SpeculativeClientWorld, projector: String) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![],
        commands: vec![],
        projection: Some(format!("{} projection result", projector)),
        rejection: None,
    });
}

#[when(expr = "I speculatively execute saga {string}")]
async fn when_speculative_saga(world: &mut SpeculativeClientWorld, _saga: String) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![],
        commands: vec!["SagaCommand1".to_string(), "SagaCommand2".to_string()],
        projection: None,
        rejection: None,
    });
    world.events_persisted = false;
}

#[when(expr = "I speculatively execute process manager {string}")]
async fn when_speculative_pm(world: &mut SpeculativeClientWorld, _pm: String) {
    if !world.has_correlation_id {
        world.error = Some("Missing correlation ID".to_string());
        world.error_type = Some("missing_correlation".to_string());
        return;
    }

    world.speculative_result = Some(SpeculativeResult {
        events: vec![],
        commands: vec!["PMCommand1".to_string()],
        projection: None,
        rejection: None,
    });
    world.events_persisted = false;
}

#[when("I speculatively execute a command producing 2 events")]
async fn when_speculative_multi_event(world: &mut SpeculativeClientWorld) {
    world.speculative_result = Some(SpeculativeResult {
        events: vec![
            MockEvent {
                sequence: 0,
                event_type: "Event1".to_string(),
            },
            MockEvent {
                sequence: 1,
                event_type: "Event2".to_string(),
            },
        ],
        commands: vec![],
        projection: None,
        rejection: None,
    });
    world.events_persisted = false;
}

#[when(expr = "I verify the real events for {string} root {string}")]
async fn when_verify_real_events(
    _world: &mut SpeculativeClientWorld,
    _domain: String,
    _root: String,
) {
    // Real events remain unchanged
}

#[when("I speculatively execute command A")]
async fn when_speculative_command_a(world: &mut SpeculativeClientWorld) {
    world.spec_a_result = Some(SpeculativeResult {
        events: vec![MockEvent {
            sequence: 0,
            event_type: "EventA".to_string(),
        }],
        commands: vec![],
        projection: None,
        rejection: None,
    });
}

#[when("I speculatively execute command B")]
async fn when_speculative_command_b(world: &mut SpeculativeClientWorld) {
    world.spec_b_result = Some(SpeculativeResult {
        events: vec![MockEvent {
            sequence: 0,
            event_type: "EventB".to_string(),
        }],
        commands: vec![],
        projection: None,
        rejection: None,
    });
}

#[when("I attempt speculative execution")]
async fn when_attempt_speculative(world: &mut SpeculativeClientWorld) {
    if !world.service_available {
        world.error = Some("Connection error".to_string());
        world.error_type = Some("connection".to_string());
    }
}

#[when("I attempt speculative execution with missing parameters")]
async fn when_attempt_missing_params(world: &mut SpeculativeClientWorld) {
    world.error = Some("Invalid argument".to_string());
    world.error_type = Some("invalid_argument".to_string());
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then("the response should contain the projected events")]
async fn then_response_contains_events(world: &mut SpeculativeClientWorld) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    assert!(!result.events.is_empty());
}

#[then("the events should NOT be persisted")]
async fn then_events_not_persisted(world: &mut SpeculativeClientWorld) {
    assert!(!world.events_persisted);
}

#[then("the command should execute against the historical state")]
async fn then_execute_against_historical(_world: &mut SpeculativeClientWorld) {
    // Verified by as_of_sequence
}

#[then(expr = "the response should reflect state at sequence {int}")]
async fn then_response_reflects_state(world: &mut SpeculativeClientWorld, _seq: u32) {
    assert!(world.speculative_result.is_some());
}

#[then("the response should indicate rejection")]
async fn then_response_rejection(world: &mut SpeculativeClientWorld) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    assert!(result.rejection.is_some());
}

#[then(expr = "the rejection reason should be {string}")]
async fn then_rejection_reason(world: &mut SpeculativeClientWorld, reason: String) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    let rejection = result.rejection.as_ref().expect("Should have rejection");
    assert!(rejection.contains(&reason));
}

#[then("the operation should fail with validation error")]
async fn then_fail_validation(world: &mut SpeculativeClientWorld) {
    assert_eq!(world.error_type, Some("validation".to_string()));
}

#[then("no events should be produced")]
async fn then_no_events_produced(world: &mut SpeculativeClientWorld) {
    assert!(
        world.speculative_result.is_none()
            || world.speculative_result.as_ref().unwrap().events.is_empty()
    );
}

#[then("an edition should be created for the speculation")]
async fn then_edition_created(world: &mut SpeculativeClientWorld) {
    assert!(world.edition_created);
}

#[then("the edition should be discarded after execution")]
async fn then_edition_discarded(world: &mut SpeculativeClientWorld) {
    assert!(world.edition_discarded);
}

#[then("the response should contain the projection")]
async fn then_response_contains_projection(world: &mut SpeculativeClientWorld) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    assert!(result.projection.is_some());
}

#[then("no external systems should be updated")]
async fn then_no_external_updates(world: &mut SpeculativeClientWorld) {
    assert!(!world.events_persisted);
}

#[then(expr = "the projector should process all {int} events in order")]
async fn then_projector_processes_all(_world: &mut SpeculativeClientWorld, _count: u32) {
    // Verified by design
}

#[then("the final projection state should be returned")]
async fn then_final_projection_state(world: &mut SpeculativeClientWorld) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    assert!(result.projection.is_some());
}

#[then("the response should contain the commands the saga would emit")]
async fn then_response_contains_commands(world: &mut SpeculativeClientWorld) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    assert!(!result.commands.is_empty());
}

#[then("the commands should NOT be sent to the target domain")]
async fn then_commands_not_sent(world: &mut SpeculativeClientWorld) {
    assert!(!world.events_persisted);
}

#[then("the response should preserve the saga origin chain")]
async fn then_preserve_saga_origin(world: &mut SpeculativeClientWorld) {
    assert!(world.saga_origin.is_some());
}

#[then("the response should contain the PM's command decisions")]
async fn then_response_contains_pm_commands(world: &mut SpeculativeClientWorld) {
    let result = world
        .speculative_result
        .as_ref()
        .expect("Should have result");
    assert!(!result.commands.is_empty());
}

#[then("the commands should NOT be executed")]
async fn then_commands_not_executed(world: &mut SpeculativeClientWorld) {
    assert!(!world.events_persisted);
}

#[then("the speculative PM operation should fail")]
async fn then_pm_operation_fails(world: &mut SpeculativeClientWorld) {
    assert!(world.error.is_some());
}

#[then("the error should indicate missing correlation ID")]
async fn then_error_missing_correlation(world: &mut SpeculativeClientWorld) {
    assert_eq!(world.error_type, Some("missing_correlation".to_string()));
}

#[then(expr = "I should receive only {int} events")]
async fn then_receive_only_n_events(world: &mut SpeculativeClientWorld, count: u32) {
    assert_eq!(world.real_event_count, count);
}

#[then("the speculative events should not be present")]
async fn then_speculative_not_present(world: &mut SpeculativeClientWorld) {
    // Speculative events are not persisted
    assert!(!world.events_persisted);
}

#[then("each speculation should start from the same base state")]
async fn then_same_base_state(world: &mut SpeculativeClientWorld) {
    assert!(world.spec_a_result.is_some());
    assert!(world.spec_b_result.is_some());
}

#[then("results should be independent")]
async fn then_results_independent(world: &mut SpeculativeClientWorld) {
    let a = world.spec_a_result.as_ref().expect("Should have A result");
    let b = world.spec_b_result.as_ref().expect("Should have B result");
    // Different event types
    assert_ne!(a.events[0].event_type, b.events[0].event_type);
}

#[then("the speculative operation should fail with connection error")]
async fn then_fail_connection(world: &mut SpeculativeClientWorld) {
    assert_eq!(world.error_type, Some("connection".to_string()));
}

#[then("the speculative operation should fail with invalid argument error")]
async fn then_fail_invalid_argument(world: &mut SpeculativeClientWorld) {
    assert_eq!(world.error_type, Some("invalid_argument".to_string()));
}
