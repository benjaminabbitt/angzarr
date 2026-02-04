//! Container integration tests using cucumber-rs.
//!
//! These tests run against a deployed angzarr system (e.g., in kind cluster).
//! Run with: ANGZARR_TEST_MODE=container cargo test --test container_integration

use std::collections::HashMap;
use std::sync::Arc;

use cucumber::{given, then, when, World};
use prost::Message;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use uuid::Uuid;

// Proto imports - generated from angzarr.proto and domains.proto
use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_query_client::EventQueryClient,
    CommandBook, CommandPage, CommandResponse, Cover, EventBook, Query, Uuid as ProtoUuid,
};

// Examples proto types
#[allow(dead_code)]
mod examples_proto {
    include!(concat!(env!("OUT_DIR"), "/examples.rs"));
}

use examples_proto::{InitializeStock, StockInitialized};

/// Container test world - connects to running gRPC services.
///
/// All commands go through the gateway, which routes to the appropriate
/// domain-specific command handler sidecar.
#[derive(World)]
#[world(init = Self::new)]
pub struct ContainerWorld {
    /// Gateway endpoint for commands (default: localhost:50051)
    gateway_endpoint: String,
    /// Event query endpoint - connects to a command sidecar (default: localhost:50052)
    query_endpoint: String,

    /// gRPC clients (lazily initialized)
    gateway_client: Option<CommandGatewayClient<Channel>>,
    query_client: Option<EventQueryClient<Channel>>,

    /// Current inventory aggregate ID
    current_inventory_id: Option<Uuid>,
    /// Named inventory IDs for multi-inventory scenarios
    named_inventories: HashMap<String, Uuid>,

    /// Last command response
    last_response: Option<CommandResponse>,
    /// Last error message
    last_error: Option<String>,

    /// Events received from query
    queried_events: Vec<EventBook>,
    /// Events received from streaming
    streamed_events: Arc<RwLock<Vec<EventBook>>>,
    /// Correlation IDs from streamed events
    correlation_ids: Arc<RwLock<Vec<String>>>,
}

impl std::fmt::Debug for ContainerWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerWorld")
            .field("gateway_endpoint", &self.gateway_endpoint)
            .field("current_inventory_id", &self.current_inventory_id)
            .finish()
    }
}

impl ContainerWorld {
    async fn new() -> Self {
        Self {
            gateway_endpoint: "http://localhost:50051".to_string(),
            query_endpoint: "http://localhost:50052".to_string(),
            gateway_client: None,
            query_client: None,
            current_inventory_id: None,
            named_inventories: HashMap::new(),
            last_response: None,
            last_error: None,
            queried_events: Vec::new(),
            streamed_events: Arc::new(RwLock::new(Vec::new())),
            correlation_ids: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn get_gateway_client(&mut self) -> &mut CommandGatewayClient<Channel> {
        if self.gateway_client.is_none() {
            let channel = Channel::from_shared(self.gateway_endpoint.clone())
                .unwrap()
                .connect()
                .await
                .expect("Failed to connect to gateway");
            self.gateway_client = Some(CommandGatewayClient::new(channel));
        }
        self.gateway_client.as_mut().unwrap()
    }

    async fn get_query_client(&mut self) -> &mut EventQueryClient<Channel> {
        if self.query_client.is_none() {
            let channel = Channel::from_shared(self.query_endpoint.clone())
                .unwrap()
                .connect()
                .await
                .expect("Failed to connect to event query");
            self.query_client = Some(EventQueryClient::new(channel));
        }
        self.query_client.as_mut().unwrap()
    }

    fn make_command_book(
        &self,
        domain: &str,
        root: Uuid,
        command: impl Message,
        type_url: &str,
    ) -> CommandBook {
        let correlation_id = Uuid::new_v4().to_string();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id,
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: format!("type.googleapis.com/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
            saga_origin: None,
        }
    }

    fn extract_event_type(event: &prost_types::Any) -> String {
        event
            .type_url
            .rsplit('/')
            .next()
            .unwrap_or(&event.type_url)
            .to_string()
    }
}

// =============================================================================
// Background Steps
// =============================================================================

#[given(expr = "the angzarr system is running at {string}")]
async fn given_system_running(world: &mut ContainerWorld, endpoint: String) {
    // Gateway handles all command routing and EventQuery
    world.gateway_endpoint = format!("http://{}", endpoint);
    // EventQuery is now proxied through the gateway
    world.query_endpoint = format!("http://{}", endpoint);

    // Verify connection by creating client
    let _ = world.get_gateway_client().await;
}

#[given(expr = "the streaming gateway is running at {string}")]
async fn given_gateway_running(world: &mut ContainerWorld, endpoint: String) {
    // This is now the same as the main gateway endpoint
    // Kept for backwards compatibility with feature files
    world.gateway_endpoint = format!("http://{}", endpoint);
}

// =============================================================================
// Inventory ID Steps
// =============================================================================

#[given("a new inventory id")]
async fn given_new_inventory_id(world: &mut ContainerWorld) {
    world.current_inventory_id = Some(Uuid::new_v4());
}

#[given(expr = "a new inventory id as {string}")]
async fn given_named_inventory_id(world: &mut ContainerWorld, name: String) {
    let id = Uuid::new_v4();
    world.named_inventories.insert(name, id);
    world.current_inventory_id = Some(id);
}

// =============================================================================
// Command Steps (via Gateway)
// =============================================================================

#[when(expr = "I send an InitializeStock command with product_id {string} and quantity {int}")]
async fn when_initialize_stock(world: &mut ContainerWorld, product_id: String, quantity: i32) {
    let inventory_id = world
        .current_inventory_id
        .expect("No inventory ID set - call 'Given a new inventory id' first");

    let command = InitializeStock {
        product_id,
        quantity,
        low_stock_threshold: 10,
    };
    let command_book =
        world.make_command_book("inventory", inventory_id, command, "examples.InitializeStock");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[given(expr = "I send an InitializeStock command with product_id {string} and quantity {int}")]
async fn given_initialize_stock(world: &mut ContainerWorld, product_id: String, quantity: i32) {
    when_initialize_stock(world, product_id, quantity).await;
}

// =============================================================================
// Query Steps
// =============================================================================

#[when("I query events for the inventory aggregate")]
async fn when_query_inventory_events(world: &mut ContainerWorld) {
    let inventory_id = world.current_inventory_id.expect("No inventory ID set");

    let query = Query {
        cover: Some(Cover {
            domain: "inventory".to_string(),
            root: Some(ProtoUuid {
                value: inventory_id.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let client = world.get_query_client().await;
    match client.get_event_book(query).await {
        Ok(response) => {
            world.queried_events = vec![response.into_inner()];
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
        }
    }
}

// =============================================================================
// Assertion Steps
// =============================================================================

#[then("the command succeeds")]
async fn then_command_succeeds(world: &mut ContainerWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected command to succeed, but got error: {:?}",
        world.last_error
    );
    assert!(
        world.last_response.is_some(),
        "Expected command response but got none"
    );
}

#[given("the command succeeds")]
async fn given_command_succeeds(world: &mut ContainerWorld) {
    then_command_succeeds(world).await;
}

#[then(expr = "the inventory aggregate has {int} event")]
async fn then_inventory_has_events(world: &mut ContainerWorld, count: usize) {
    then_aggregate_has_events(world, "inventory", count).await;
}

#[then(expr = "the inventory aggregate has {int} events")]
async fn then_inventory_has_multiple_events(world: &mut ContainerWorld, count: usize) {
    then_inventory_has_events(world, count).await;
}

async fn then_aggregate_has_events(world: &mut ContainerWorld, domain: &str, expected: usize) {
    // Small delay to ensure write is committed and visible
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let root = match domain {
        "inventory" => world.current_inventory_id,
        _ => panic!("Unknown domain: {}", domain),
    }
    .expect("No aggregate ID set");

    let query = Query {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let client = world.get_query_client().await;
    let response = client
        .get_event_book(query)
        .await
        .expect("Failed to query events");
    let event_book = response.into_inner();
    let actual = event_book.pages.len();

    assert_eq!(
        actual, expected,
        "Expected {} events for {} aggregate, got {}",
        expected, domain, actual
    );
}

#[then(expr = "the latest event type is {string}")]
async fn then_latest_event_type(world: &mut ContainerWorld, expected_type: String) {
    let response = world
        .last_response
        .as_ref()
        .expect("No command response available");
    let events = response.events.as_ref().expect("No events in response");

    assert!(!events.pages.is_empty(), "No events in response");

    let last_event = events.pages.last().unwrap();
    let event_any = last_event.event.as_ref().expect("Event has no payload");
    let actual_type = ContainerWorld::extract_event_type(event_any);

    assert!(
        actual_type.contains(&expected_type),
        "Expected event type containing '{}', got '{}'",
        expected_type,
        actual_type
    );
}

#[then(expr = "a projection was returned from projector {string}")]
async fn then_projection_returned(world: &mut ContainerWorld, projector_name: String) {
    let response = world
        .last_response
        .as_ref()
        .expect("No command response available");

    let projection = response
        .projections
        .iter()
        .find(|p| p.projector == projector_name);

    assert!(
        projection.is_some(),
        "Expected projection from '{}', but found: {:?}",
        projector_name,
        response
            .projections
            .iter()
            .map(|p| &p.projector)
            .collect::<Vec<_>>()
    );
}

#[then(expr = "I receive {int} event")]
async fn then_receive_event(world: &mut ContainerWorld, count: usize) {
    then_receive_events(world, count).await;
}

#[then(expr = "I receive {int} events")]
async fn then_receive_events(world: &mut ContainerWorld, count: usize) {
    assert_eq!(
        world.queried_events.len(),
        1,
        "Expected 1 event book from query"
    );
    let event_book = &world.queried_events[0];
    assert_eq!(
        event_book.pages.len(),
        count,
        "Expected {} events, got {}",
        count,
        event_book.pages.len()
    );
}

#[then(expr = "the event at sequence {int} has type {string}")]
async fn then_event_at_sequence(
    world: &mut ContainerWorld,
    sequence: usize,
    expected_type: String,
) {
    let event_book = &world.queried_events[0];
    let event = &event_book.pages[sequence];
    let event_any = event.event.as_ref().expect("Event has no payload");
    let actual_type = ContainerWorld::extract_event_type(event_any);

    assert!(
        actual_type.contains(&expected_type),
        "Expected event type containing '{}' at sequence {}, got '{}'",
        expected_type,
        sequence,
        actual_type
    );
}

// =============================================================================
// Gateway Streaming Steps
// =============================================================================

#[when(expr = "I send an InitializeStock command via gateway with product_id {string} and quantity {int}")]
async fn when_initialize_stock_via_gateway(
    world: &mut ContainerWorld,
    product_id: String,
    quantity: i32,
) {
    let inventory_id = world.current_inventory_id.expect("No inventory ID set");

    let command = InitializeStock {
        product_id,
        quantity,
        low_stock_threshold: 10,
    };
    let command_book =
        world.make_command_book("inventory", inventory_id, command, "examples.InitializeStock");

    // Store correlation ID for later verification
    let correlation_id = command_book
        .cover
        .as_ref()
        .map(|c| c.correlation_id.clone())
        .unwrap_or_default();
    world.correlation_ids.write().await.push(correlation_id);

    let client = world.get_gateway_client().await;
    match client.execute_stream(command_book).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let events = world.streamed_events.clone();
            // Collect events with timeout
            let timeout = tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
                while let Ok(Some(event_book)) = stream.message().await {
                    events.write().await.push(event_book);
                }
            });
            let _ = timeout.await;
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
        }
    }
}

#[given(expr = "I send an InitializeStock command via gateway with product_id {string} and quantity {int}")]
async fn given_initialize_stock_via_gateway(
    world: &mut ContainerWorld,
    product_id: String,
    quantity: i32,
) {
    when_initialize_stock_via_gateway(world, product_id, quantity).await;
}

#[when(
    expr = "I send an InitializeStock command via gateway for {string} with product_id {string} and quantity {int}"
)]
async fn when_initialize_stock_via_gateway_named(
    world: &mut ContainerWorld,
    inventory_name: String,
    product_id: String,
    quantity: i32,
) {
    let inventory_id = world
        .named_inventories
        .get(&inventory_name)
        .copied()
        .expect("Named inventory not found");

    world.current_inventory_id = Some(inventory_id);
    when_initialize_stock_via_gateway(world, product_id, quantity).await;
}

#[when("I subscribe to events with a non-matching correlation ID")]
async fn when_subscribe_non_matching(world: &mut ContainerWorld) {
    // Create a command with a random correlation ID that won't match any events
    let inventory_id = world.current_inventory_id.unwrap_or_else(Uuid::new_v4);
    let command = InitializeStock {
        product_id: "NON-MATCH-SKU".to_string(),
        quantity: 1,
        low_stock_threshold: 1,
    };
    let mut command_book =
        world.make_command_book("inventory", inventory_id, command, "examples.InitializeStock");
    if let Some(ref mut cover) = command_book.cover {
        cover.correlation_id = "non-matching-correlation-id".to_string();
    }

    // This should timeout or return empty
    let client = world.get_gateway_client().await;
    let _ = client.execute_stream(command_book).await;
}

#[then(expr = "I receive at least {int} event from the stream")]
async fn then_receive_streamed_event(world: &mut ContainerWorld, min_count: usize) {
    then_receive_streamed_events(world, min_count).await;
}

#[then(expr = "I receive at least {int} events from the stream")]
async fn then_receive_streamed_events(world: &mut ContainerWorld, min_count: usize) {
    let events = world.streamed_events.read().await;
    assert!(
        events.len() >= min_count,
        "Expected at least {} streamed events, got {}",
        min_count,
        events.len()
    );
}

#[then(expr = "the streamed events include type {string}")]
async fn then_streamed_events_include_type(world: &mut ContainerWorld, expected_type: String) {
    let events = world.streamed_events.read().await;
    let has_type = events.iter().any(|book| {
        book.pages.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| ContainerWorld::extract_event_type(e).contains(&expected_type))
                .unwrap_or(false)
        })
    });

    assert!(
        has_type,
        "Expected streamed events to include type '{}', but got types: {:?}",
        expected_type,
        events
            .iter()
            .flat_map(|b| b.pages.iter())
            .filter_map(|p| p.event.as_ref())
            .map(ContainerWorld::extract_event_type)
            .collect::<Vec<_>>()
    );
}

#[then("all streamed events have the same correlation ID")]
async fn then_same_correlation_id(world: &mut ContainerWorld) {
    let events = world.streamed_events.read().await;
    if events.is_empty() {
        return; // No events to check
    }

    let first_corr_id = events[0]
        .cover
        .as_ref()
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");
    for event in events.iter() {
        let event_corr_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_corr_id, first_corr_id,
            "Expected all events to have correlation ID '{}', but found '{}'",
            first_corr_id, event_corr_id
        );
    }
}

#[then(expr = "events for {string} only contain product_id {string}")]
async fn then_events_for_inventory_contain(
    world: &mut ContainerWorld,
    inventory_name: String,
    expected_product_id: String,
) {
    // Get the inventory ID for this named inventory
    let inventory_id = world
        .named_inventories
        .get(&inventory_name)
        .copied()
        .expect("Named inventory not found");

    let events = world.streamed_events.read().await;

    // Find StockInitialized events for this specific inventory and verify product_id
    let mut found_event = false;
    for book in events.iter() {
        // Check if this event book is for our inventory
        if let Some(cover) = &book.cover {
            if let Some(root) = &cover.root {
                let event_root = uuid::Uuid::from_slice(&root.value).ok();
                if event_root == Some(inventory_id) {
                    for page in &book.pages {
                        if let Some(event_any) = &page.event {
                            let type_name = ContainerWorld::extract_event_type(event_any);
                            if type_name.contains("StockInitialized") {
                                let initialized =
                                    StockInitialized::decode(event_any.value.as_slice())
                                        .expect("Failed to decode StockInitialized");
                                assert!(
                                    initialized.product_id.contains(&expected_product_id),
                                    "Expected inventory {} product_id to contain '{}', got '{}'",
                                    inventory_name,
                                    expected_product_id,
                                    initialized.product_id
                                );
                                found_event = true;
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        found_event,
        "No StockInitialized event found for inventory '{}'",
        inventory_name
    );
}

#[then("the stream closes after the timeout period")]
async fn then_stream_closes_timeout(_world: &mut ContainerWorld) {
    // The stream should have closed - if we got here without hanging, the test passes
}

#[tokio::main]
async fn main() {
    // Only run if ANGZARR_TEST_MODE=container
    let run_container = std::env::var("ANGZARR_TEST_MODE")
        .map(|v| v.to_lowercase() == "container")
        .unwrap_or(false);

    if !run_container {
        println!("Skipping container tests. Set ANGZARR_TEST_MODE=container to run.");
        return;
    }

    ContainerWorld::cucumber()
        .filter_run("tests/acceptance/features", |feature, _, sc| {
            feature.tags.iter().any(|t| t == "container")
                || sc.tags.iter().any(|t| t == "container")
        })
        .await;
}
