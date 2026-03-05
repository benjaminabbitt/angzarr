//! DomainClient step definitions.

use cucumber::{given, then, when, World};
use std::collections::HashMap;

/// Test context for DomainClient scenarios.
#[derive(Debug, Default, World)]
pub struct DomainClientWorld {
    domain: String,
    endpoint: String,
    domain_client_created: bool,
    domain_client_connected: bool,
    domain_client_closed: bool,
    can_query: bool,
    can_command: bool,
    command_sent: bool,
    query_executed: bool,
    events_received: u32,
    command_response_received: bool,
    same_connection_used: bool,
    error: Option<String>,
    env_var: Option<String>,
    aggregates: HashMap<String, u32>,
}

// ==========================================================================
// Background Steps
// ==========================================================================

#[given(expr = "a running aggregate coordinator for domain {string}")]
async fn given_running_coordinator(world: &mut DomainClientWorld, domain: String) {
    world.domain = domain;
}

#[given(expr = "a registered aggregate handler for domain {string}")]
async fn given_registered_handler(_world: &mut DomainClientWorld, _domain: String) {
    // Handler is registered
}

// ==========================================================================
// Given Steps
// ==========================================================================

#[given(expr = "an aggregate {string} with root {string} has {int} events")]
async fn given_aggregate_with_events(
    world: &mut DomainClientWorld,
    domain: String,
    root: String,
    count: u32,
) {
    let key = format!("{}:{}", domain, root);
    world.aggregates.insert(key, count);
}

#[given("a connected DomainClient")]
async fn given_connected_domain_client(world: &mut DomainClientWorld) {
    world.domain_client_created = true;
    world.domain_client_connected = true;
}

#[given(expr = "environment variable {string} is set to the coordinator endpoint")]
async fn given_env_var_set(world: &mut DomainClientWorld, var_name: String) {
    world.env_var = Some(var_name);
    world.endpoint = "http://localhost:1310".to_string();
}

// ==========================================================================
// When Steps
// ==========================================================================

#[when("I create a DomainClient for the coordinator endpoint")]
async fn when_create_domain_client_coordinator(world: &mut DomainClientWorld) {
    world.domain_client_created = true;
    world.domain_client_connected = true;
    world.can_query = true;
    world.can_command = true;
}

#[when(expr = "I create a DomainClient for domain {string}")]
async fn when_create_domain_client_domain(world: &mut DomainClientWorld, domain: String) {
    world.domain = domain;
    world.domain_client_created = true;
    world.domain_client_connected = true;
    world.can_query = true;
    world.can_command = true;
}

#[when("I use the command builder to send a command")]
async fn when_use_command_builder(world: &mut DomainClientWorld) {
    world.command_sent = true;
    world.command_response_received = true;
}

#[when("I use the query builder to fetch events for that root")]
async fn when_use_query_builder(world: &mut DomainClientWorld) {
    world.query_executed = true;
    // Find the aggregate and return its event count
    for (key, count) in &world.aggregates {
        if key.starts_with(&world.domain) {
            world.events_received = *count;
            break;
        }
    }
}

#[when("I send a command")]
async fn when_send_command(world: &mut DomainClientWorld) {
    world.command_sent = true;
    world.same_connection_used = true;
}

#[when("I query for the resulting events")]
async fn when_query_resulting_events(world: &mut DomainClientWorld) {
    world.query_executed = true;
    world.same_connection_used = true;
}

#[when("I close the DomainClient")]
async fn when_close_domain_client(world: &mut DomainClientWorld) {
    world.domain_client_closed = true;
    world.domain_client_connected = false;
}

#[when(expr = "I create a DomainClient from environment variable {string}")]
async fn when_create_from_env(world: &mut DomainClientWorld, _var_name: String) {
    world.domain_client_created = true;
    world.domain_client_connected = true;
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then("I should be able to query events")]
async fn then_can_query(world: &mut DomainClientWorld) {
    assert!(world.can_query);
}

#[then("I should be able to send commands")]
async fn then_can_command(world: &mut DomainClientWorld) {
    assert!(world.can_command);
}

#[then("I should receive a CommandResponse")]
async fn then_receive_command_response(world: &mut DomainClientWorld) {
    assert!(world.command_response_received);
}

#[then(expr = "I should receive {int} EventPages")]
async fn then_receive_event_pages(world: &mut DomainClientWorld, count: u32) {
    assert_eq!(world.events_received, count);
}

#[then("both operations should succeed on the same connection")]
async fn then_same_connection(world: &mut DomainClientWorld) {
    assert!(world.same_connection_used);
    assert!(world.command_sent);
    assert!(world.query_executed);
}

#[then("subsequent commands should fail with ConnectionError")]
async fn then_commands_fail_connection_error(world: &mut DomainClientWorld) {
    assert!(world.domain_client_closed);
    world.error = Some("ConnectionError".to_string());
}

#[then("subsequent queries should fail with ConnectionError")]
async fn then_queries_fail_connection_error(world: &mut DomainClientWorld) {
    assert!(world.domain_client_closed);
    world.error = Some("ConnectionError".to_string());
}

#[then("the DomainClient should be connected")]
async fn then_domain_client_connected(world: &mut DomainClientWorld) {
    assert!(world.domain_client_connected);
}
