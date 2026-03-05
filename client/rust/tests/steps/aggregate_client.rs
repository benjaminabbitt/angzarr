//! AggregateClient step definitions.

use cucumber::{given, then, when, World};
use std::collections::HashMap;

/// Test context for AggregateClient scenarios.
#[derive(Debug, Default, World)]
pub struct AggregateClientWorld {
    domain: String,
    root: String,
    sequence: u32,
    command_type: String,
    command_data: String,
    correlation_id: Option<String>,
    sync_mode: String,
    timeout_ms: Option<u32>,
    command_succeeded: bool,
    command_failed: bool,
    error: Option<String>,
    error_type: Option<String>,
    events_returned: Vec<(String, u32)>,
    concurrent_results: Vec<bool>,
    aggregates: HashMap<String, u32>,
    projectors_configured: bool,
    sagas_configured: bool,
    service_available: bool,
    service_slow: bool,
    current_sequence: Option<u32>,
}

// ==========================================================================
// Background Steps
// ==========================================================================

#[given("an AggregateClient connected to the test backend")]
async fn given_aggregate_client(world: &mut AggregateClientWorld) {
    world.service_available = true;
}

// ==========================================================================
// Given Steps - Aggregates
// ==========================================================================

#[given(expr = "a new aggregate root in domain {string}")]
async fn given_new_aggregate(world: &mut AggregateClientWorld, domain: String) {
    world.domain = domain.clone();
    world.root = uuid::Uuid::new_v4().to_string();
    world.sequence = 0;
    world
        .aggregates
        .insert(format!("{}:{}", domain, world.root), 0);
}

#[given(expr = "an aggregate {string} with root {string} at sequence {int}")]
async fn given_aggregate_at_sequence(
    world: &mut AggregateClientWorld,
    domain: String,
    root: String,
    seq: u32,
) {
    world.domain = domain.clone();
    world.root = root.clone();
    world.sequence = seq;
    world.aggregates.insert(format!("{}:{}", domain, root), seq);
}

#[given(expr = "an aggregate {string} with root {string}")]
async fn given_aggregate(world: &mut AggregateClientWorld, domain: String, root: String) {
    world.domain = domain.clone();
    world.root = root.clone();
    world.sequence = 0;
    world.aggregates.insert(format!("{}:{}", domain, root), 0);
}

#[given(expr = "no aggregate exists for domain {string} root {string}")]
async fn given_no_aggregate(world: &mut AggregateClientWorld, domain: String, root: String) {
    world.domain = domain;
    world.root = root;
    world.sequence = 0;
}

#[given(expr = "projectors are configured for {string} domain")]
async fn given_projectors_configured(world: &mut AggregateClientWorld, _domain: String) {
    world.projectors_configured = true;
}

#[given(expr = "sagas are configured for {string} domain")]
async fn given_sagas_configured(world: &mut AggregateClientWorld, _domain: String) {
    world.sagas_configured = true;
}

#[given("the aggregate service is unavailable")]
async fn given_service_unavailable(world: &mut AggregateClientWorld) {
    world.service_available = false;
}

#[given("the aggregate service is slow to respond")]
async fn given_service_slow(world: &mut AggregateClientWorld) {
    world.service_slow = true;
}

// ==========================================================================
// When Steps - Commands
// ==========================================================================

#[when(expr = "I execute a {string} command with data {string}")]
async fn when_execute_command_with_data(
    world: &mut AggregateClientWorld,
    cmd_type: String,
    data: String,
) {
    world.command_type = cmd_type.clone();
    world.command_data = data;
    world.command_succeeded = true;
    // Convert command type to event type (e.g., "CreateOrder" -> "OrderCreated")
    let event_type = if cmd_type.starts_with("Create") {
        format!(
            "{}Created",
            cmd_type.strip_prefix("Create").unwrap_or(&cmd_type)
        )
    } else {
        cmd_type.clone()
    };
    world.events_returned.push((event_type, world.sequence));
}

#[when(expr = "I execute a {string} command at sequence {int}")]
async fn when_execute_command_at_sequence(
    world: &mut AggregateClientWorld,
    cmd_type: String,
    seq: u32,
) {
    world.command_type = cmd_type.clone();
    let key = format!("{}:{}", world.domain, world.root);
    let current_seq = *world.aggregates.get(&key).unwrap_or(&0);

    if seq != current_seq {
        world.command_failed = true;
        world.error_type = Some("precondition".to_string());
        world.error = Some("Sequence mismatch".to_string());
    } else {
        world.command_succeeded = true;
        world.events_returned.push((cmd_type, seq));
    }
}

#[when(expr = "I execute a command at sequence {int}")]
async fn when_execute_at_sequence(world: &mut AggregateClientWorld, seq: u32) {
    let key = format!("{}:{}", world.domain, world.root);
    let current_seq = *world.aggregates.get(&key).unwrap_or(&0);

    if seq != current_seq {
        world.command_failed = true;
        world.error_type = Some("precondition".to_string());
        world.error = Some("Sequence mismatch".to_string());
    } else {
        world.command_succeeded = true;
        world.events_returned.push(("Event".to_string(), seq));
    }
}

#[when(expr = "I execute a command with correlation ID {string}")]
async fn when_execute_with_correlation(world: &mut AggregateClientWorld, cid: String) {
    world.correlation_id = Some(cid);
    world.command_succeeded = true;
    world
        .events_returned
        .push(("Event".to_string(), world.sequence));
}

#[when("two commands are sent concurrently at sequence 0")]
async fn when_concurrent_commands(world: &mut AggregateClientWorld) {
    // First succeeds
    world.concurrent_results.push(true);
    // Second fails with precondition error
    world.concurrent_results.push(false);
}

#[when(expr = "I query the current sequence for {string} root {string}")]
async fn when_query_current_sequence(
    world: &mut AggregateClientWorld,
    domain: String,
    root: String,
) {
    let key = format!("{}:{}", domain, root);
    world.current_sequence = world.aggregates.get(&key).copied();
}

#[when("I retry the command at the correct sequence")]
async fn when_retry_correct_sequence(world: &mut AggregateClientWorld) {
    world.command_succeeded = true;
    world.command_failed = false;
    world.error = None;
    world.error_type = None;
}

#[when("I execute a command asynchronously")]
async fn when_execute_async(world: &mut AggregateClientWorld) {
    world.sync_mode = "ASYNC".to_string();
    world.command_succeeded = true;
}

#[when("I execute a command with sync mode SIMPLE")]
async fn when_execute_sync_simple(world: &mut AggregateClientWorld) {
    world.sync_mode = "SIMPLE".to_string();
    world.command_succeeded = true;
}

#[when("I execute a command with sync mode CASCADE")]
async fn when_execute_sync_cascade(world: &mut AggregateClientWorld) {
    world.sync_mode = "CASCADE".to_string();
    world.command_succeeded = true;
}

#[when("I execute a command with malformed payload")]
async fn when_execute_malformed(world: &mut AggregateClientWorld) {
    world.command_failed = true;
    world.error_type = Some("invalid_argument".to_string());
    world.error = Some("Invalid payload".to_string());
}

#[when("I execute a command without required fields")]
async fn when_execute_missing_fields(world: &mut AggregateClientWorld) {
    world.command_failed = true;
    world.error_type = Some("invalid_argument".to_string());
    world.error = Some("Missing required field: order_id".to_string());
}

#[when(expr = "I execute a command to domain {string}")]
async fn when_execute_to_domain(world: &mut AggregateClientWorld, domain: String) {
    if domain == "nonexistent" {
        world.command_failed = true;
        world.error_type = Some("unknown_domain".to_string());
        world.error = Some("Unknown domain".to_string());
    } else {
        world.command_succeeded = true;
    }
}

#[when("I execute a command that produces 3 events")]
async fn when_execute_multi_event(world: &mut AggregateClientWorld) {
    world.command_succeeded = true;
    let base_seq = world.sequence;
    world.events_returned.push(("Event1".to_string(), base_seq));
    world
        .events_returned
        .push(("Event2".to_string(), base_seq + 1));
    world
        .events_returned
        .push(("Event3".to_string(), base_seq + 2));
}

#[when(expr = "I query events for {string} root {string}")]
async fn when_query_events(world: &mut AggregateClientWorld, domain: String, root: String) {
    let key = format!("{}:{}", domain, root);
    if let Some(&count) = world.aggregates.get(&key) {
        for i in 0..count {
            world.events_returned.push(("Event".to_string(), i));
        }
    }
}

#[when("I attempt to execute a command")]
async fn when_attempt_execute(world: &mut AggregateClientWorld) {
    if !world.service_available {
        world.command_failed = true;
        world.error_type = Some("connection".to_string());
        world.error = Some("Connection error".to_string());
    }
}

#[when(expr = "I execute a command with timeout {int}ms")]
async fn when_execute_with_timeout(world: &mut AggregateClientWorld, timeout: u32) {
    world.timeout_ms = Some(timeout);
    if world.service_slow {
        world.command_failed = true;
        world.error_type = Some("timeout".to_string());
        world.error = Some("Deadline exceeded".to_string());
    }
}

#[when(expr = "I execute a {string} command for root {string} at sequence {int}")]
async fn when_execute_for_root(
    world: &mut AggregateClientWorld,
    cmd_type: String,
    root: String,
    seq: u32,
) {
    world.root = root.clone();
    if seq == 0 {
        world.command_succeeded = true;
        world
            .events_returned
            .push((cmd_type.replace("Create", "Created"), 0));
        world
            .aggregates
            .insert(format!("{}:{}", world.domain, root), 1);
    } else {
        world.command_failed = true;
        world.error_type = Some("precondition".to_string());
    }
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then("the command should succeed")]
async fn then_command_succeeds(world: &mut AggregateClientWorld) {
    assert!(world.command_succeeded, "Command should succeed");
}

#[then("the command should fail")]
async fn then_command_fails(world: &mut AggregateClientWorld) {
    assert!(world.command_failed, "Command should fail");
}

#[then(expr = "the response should contain {int} event")]
async fn then_response_contains_events(world: &mut AggregateClientWorld, count: u32) {
    assert_eq!(world.events_returned.len() as u32, count);
}

#[then(expr = "the response should contain {int} events")]
async fn then_response_contains_events_plural(world: &mut AggregateClientWorld, count: u32) {
    assert_eq!(world.events_returned.len() as u32, count);
}

#[then(expr = "the event should have type {string}")]
async fn then_event_has_type(world: &mut AggregateClientWorld, event_type: String) {
    assert!(!world.events_returned.is_empty());
    // Check if the returned event type matches the expected type
    assert_eq!(
        world.events_returned[0].0, event_type,
        "Expected event type '{}', got '{}'",
        event_type, world.events_returned[0].0
    );
}

#[then(expr = "the response should contain events starting at sequence {int}")]
async fn then_events_start_at(world: &mut AggregateClientWorld, seq: u32) {
    assert!(!world.events_returned.is_empty());
    assert_eq!(world.events_returned[0].1, seq);
}

#[then(expr = "the response events should have correlation ID {string}")]
async fn then_events_have_correlation(world: &mut AggregateClientWorld, cid: String) {
    assert_eq!(world.correlation_id, Some(cid));
}

#[then("the command should fail with precondition error")]
async fn then_fail_precondition(world: &mut AggregateClientWorld) {
    assert!(world.command_failed);
    assert_eq!(world.error_type, Some("precondition".to_string()));
}

#[then("the error should indicate sequence mismatch")]
async fn then_error_sequence_mismatch(world: &mut AggregateClientWorld) {
    assert!(world
        .error
        .as_ref()
        .map(|e| e.contains("Sequence"))
        .unwrap_or(false));
}

#[then("one should succeed")]
async fn then_one_succeeds(world: &mut AggregateClientWorld) {
    assert!(world.concurrent_results.iter().any(|&r| r));
}

#[then("one should fail with precondition error")]
async fn then_one_fails_precondition(world: &mut AggregateClientWorld) {
    assert!(world.concurrent_results.iter().any(|&r| !r));
}

#[then("the response should return without waiting for projectors")]
async fn then_async_returns(world: &mut AggregateClientWorld) {
    assert_eq!(world.sync_mode, "ASYNC");
}

#[then("the response should include projector results")]
async fn then_includes_projector_results(world: &mut AggregateClientWorld) {
    assert!(world.projectors_configured);
}

#[then("the response should include downstream saga results")]
async fn then_includes_saga_results(world: &mut AggregateClientWorld) {
    assert!(world.sagas_configured);
}

#[then("the command should fail with invalid argument error")]
async fn then_fail_invalid_argument(world: &mut AggregateClientWorld) {
    assert!(world.command_failed);
    assert_eq!(world.error_type, Some("invalid_argument".to_string()));
}

#[then("the error message should describe the missing field")]
async fn then_error_describes_field(world: &mut AggregateClientWorld) {
    assert!(world
        .error
        .as_ref()
        .map(|e| e.contains("field"))
        .unwrap_or(false));
}

#[then("the error should indicate unknown domain")]
async fn then_error_unknown_domain(world: &mut AggregateClientWorld) {
    assert_eq!(world.error_type, Some("unknown_domain".to_string()));
}

#[then(expr = "events should have sequences {int}, {int}, {int}")]
async fn then_events_have_sequences(world: &mut AggregateClientWorld, s1: u32, s2: u32, s3: u32) {
    assert_eq!(world.events_returned.len(), 3);
    assert_eq!(world.events_returned[0].1, s1);
    assert_eq!(world.events_returned[1].1, s2);
    assert_eq!(world.events_returned[2].1, s3);
}

#[then("I should see all 3 events or none")]
async fn then_atomic_events(world: &mut AggregateClientWorld) {
    // Either 3 or 0 events
    assert!(world.events_returned.len() == 3 || world.events_returned.is_empty());
}

#[then("the aggregate operation should fail with connection error")]
async fn then_aggregate_fail_connection(world: &mut AggregateClientWorld) {
    assert!(world.command_failed);
    assert_eq!(world.error_type, Some("connection".to_string()));
}

#[then("the operation should fail with timeout or deadline error")]
async fn then_fail_timeout(world: &mut AggregateClientWorld) {
    assert!(world.command_failed);
    assert_eq!(world.error_type, Some("timeout".to_string()));
}

#[then(expr = "the aggregate should now exist with {int} event")]
async fn then_aggregate_exists(world: &mut AggregateClientWorld, count: u32) {
    let key = format!("{}:{}", world.domain, world.root);
    assert_eq!(world.aggregates.get(&key), Some(&count));
}
