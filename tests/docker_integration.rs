//! Integration tests against running podman-compose services.
//!
//! Run with: cargo test --test docker_integration
//! Requires: podman-compose --profile rust up -d
//!
//! Environment variables:
//!   EVENTED_COMMAND_URL - evented command handler URL (default: http://localhost:50051)
//!   EVENTED_QUERY_URL - evented event query URL (default: http://localhost:50052)

use std::collections::HashMap;
use std::env;

use cucumber::{given, then, when, World};
use prost::Message;
use tonic::transport::Channel;
use uuid::Uuid;

use evented::proto::{
    business_coordinator_client::BusinessCoordinatorClient,
    event_query_client::EventQueryClient,
    CommandBook, CommandPage, Cover, Query, Uuid as ProtoUuid,
};

use common::proto::{CreateCustomer, CreateTransaction, CompleteTransaction, LineItem, Receipt};

/// Integration test world state.
#[derive(World)]
#[world(init = Self::new)]
pub struct IntegrationWorld {
    /// gRPC client for business coordinator.
    business_client: Option<BusinessCoordinatorClient<Channel>>,
    /// gRPC client for event queries.
    query_client: Option<EventQueryClient<Channel>>,
    /// Current customer UUID.
    customer_id: Option<Uuid>,
    /// Current transaction UUID.
    transaction_id: Option<Uuid>,
    /// Last command response.
    last_response: Option<evented::proto::SynchronousProcessingResponse>,
    /// Last error message.
    last_error: Option<String>,
    /// Queried events.
    queried_events: Vec<evented::proto::EventPage>,
    /// Service endpoints.
    endpoints: HashMap<String, String>,
}

impl std::fmt::Debug for IntegrationWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationWorld")
            .field("customer_id", &self.customer_id)
            .field("transaction_id", &self.transaction_id)
            .finish()
    }
}

impl IntegrationWorld {
    async fn new() -> Self {
        Self {
            business_client: None,
            query_client: None,
            customer_id: None,
            transaction_id: None,
            last_response: None,
            last_error: None,
            queried_events: Vec::new(),
            endpoints: HashMap::new(),
        }
    }

    fn make_proto_uuid(uuid: Uuid) -> ProtoUuid {
        ProtoUuid {
            value: uuid.as_bytes().to_vec(),
        }
    }

    async fn send_command(&mut self, domain: &str, root: Uuid, command: prost_types::Any) {
        let client = self.business_client.as_mut().expect("Client not connected");

        let command_book = CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(Self::make_proto_uuid(root)),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                synchronous: false,
                command: Some(command),
            }],
        };

        match client.handle(tonic::Request::new(command_book)).await {
            Ok(response) => {
                self.last_response = Some(response.into_inner());
                self.last_error = None;
            }
            Err(status) => {
                self.last_error = Some(status.message().to_string());
                self.last_response = None;
            }
        }
    }
}

// Step implementations

#[given(expr = "the evented system is running at {string}")]
async fn given_evented_running(world: &mut IntegrationWorld, default_endpoint: String) {
    // Allow override via environment variables
    let command_addr = env::var("EVENTED_COMMAND_URL")
        .unwrap_or_else(|_| format!("http://{}", default_endpoint));
    let query_addr = env::var("EVENTED_QUERY_URL")
        .unwrap_or_else(|_| "http://localhost:50052".to_string());

    world.endpoints.insert("evented_command".to_string(), command_addr.clone());
    world.endpoints.insert("evented_query".to_string(), query_addr.clone());

    // Connect to business coordinator
    let command_channel = Channel::from_shared(command_addr.clone())
        .expect("Invalid command endpoint")
        .connect()
        .await
        .expect(&format!("Failed to connect to evented command at {}", command_addr));

    // Connect to event query (different port)
    let query_channel = Channel::from_shared(query_addr.clone())
        .expect("Invalid query endpoint")
        .connect()
        .await
        .expect(&format!("Failed to connect to evented query at {}", query_addr));

    world.business_client = Some(BusinessCoordinatorClient::new(command_channel));
    world.query_client = Some(EventQueryClient::new(query_channel));
}


#[given("a new customer id")]
async fn given_new_customer_id(world: &mut IntegrationWorld) {
    world.customer_id = Some(Uuid::new_v4());
}

#[given("a new transaction id for the customer")]
async fn given_new_transaction_id(world: &mut IntegrationWorld) {
    world.transaction_id = Some(Uuid::new_v4());
}

#[given(expr = "I send a CreateCustomer command with name {string} and email {string}")]
async fn given_create_customer(world: &mut IntegrationWorld, name: String, email: String) {
    when_create_customer(world, name, email).await;
}

#[when(expr = "I send a CreateCustomer command with name {string} and email {string}")]
async fn when_create_customer(world: &mut IntegrationWorld, name: String, email: String) {
    let customer_id = world.customer_id.expect("Customer ID not set");

    let cmd = CreateCustomer { name, email };

    let command = prost_types::Any {
        type_url: "type.examples/examples.CreateCustomer".to_string(),
        value: cmd.encode_to_vec(),
    };

    world.send_command("customer", customer_id, command).await;
}

#[when("I send a CreateTransaction command with items:")]
async fn when_create_transaction(world: &mut IntegrationWorld, step: &cucumber::gherkin::Step) {
    let transaction_id = world.transaction_id.expect("Transaction ID not set");
    let customer_id = world.customer_id.expect("Customer ID not set");

    let mut items = Vec::new();
    if let Some(table) = &step.table {
        for row in table.rows.iter().skip(1) {
            items.push(LineItem {
                product_id: row[0].clone(),
                name: row[1].clone(),
                quantity: row[2].parse().unwrap(),
                unit_price_cents: row[3].parse().unwrap(),
            });
        }
    }

    let cmd = CreateTransaction {
        customer_id: customer_id.to_string(),
        items,
    };

    let command = prost_types::Any {
        type_url: "type.examples/examples.CreateTransaction".to_string(),
        value: cmd.encode_to_vec(),
    };

    world
        .send_command("transaction", transaction_id, command)
        .await;
}

#[when(expr = "I send a CompleteTransaction command with payment method {string}")]
async fn when_complete_transaction(world: &mut IntegrationWorld, payment_method: String) {
    let transaction_id = world.transaction_id.expect("Transaction ID not set");

    let cmd = CompleteTransaction { payment_method };

    let command = prost_types::Any {
        type_url: "type.examples/examples.CompleteTransaction".to_string(),
        value: cmd.encode_to_vec(),
    };

    world
        .send_command("transaction", transaction_id, command)
        .await;
}

#[when("I query events for the customer aggregate")]
async fn when_query_customer_events(world: &mut IntegrationWorld) {
    let customer_id = world.customer_id.expect("Customer ID not set");
    let client = world.query_client.as_mut().expect("Query client not connected");

    let query = Query {
        domain: "customer".to_string(),
        root: Some(IntegrationWorld::make_proto_uuid(customer_id)),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };

    let mut stream = client
        .get_events(tonic::Request::new(query))
        .await
        .expect("Query failed")
        .into_inner();

    world.queried_events.clear();
    while let Some(book) = stream.message().await.expect("Stream error") {
        world.queried_events.extend(book.pages);
    }
}

#[given("the command succeeds")]
async fn given_command_succeeds(world: &mut IntegrationWorld) {
    assert_command_succeeds(world);
}

#[then("the command succeeds")]
async fn then_command_succeeds(world: &mut IntegrationWorld) {
    assert_command_succeeds(world);
}

fn assert_command_succeeds(world: &IntegrationWorld) {
    assert!(
        world.last_error.is_none(),
        "Command failed: {:?}",
        world.last_error
    );
    assert!(
        world.last_response.is_some(),
        "No response received"
    );
}

#[then(expr = "the customer aggregate has {int} event")]
async fn then_customer_has_event(world: &mut IntegrationWorld, count: usize) {
    then_customer_has_events(world, count).await;
}

#[then(expr = "the customer aggregate has {int} events")]
async fn then_customer_has_events(world: &mut IntegrationWorld, expected: usize) {
    let customer_id = world.customer_id.expect("Customer ID not set");
    let client = world.query_client.as_mut().expect("Query client not connected");

    let query = Query {
        domain: "customer".to_string(),
        root: Some(IntegrationWorld::make_proto_uuid(customer_id)),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };

    let mut stream = client
        .get_events(tonic::Request::new(query))
        .await
        .expect("Query failed")
        .into_inner();

    let mut total_events = 0;
    while let Some(book) = stream.message().await.expect("Stream error") {
        total_events += book.pages.len();
    }

    assert_eq!(
        total_events, expected,
        "Expected {} events, got {}",
        expected, total_events
    );
}

#[then(expr = "the transaction aggregate has {int} event")]
async fn then_transaction_has_event(world: &mut IntegrationWorld, count: usize) {
    then_transaction_has_events(world, count).await;
}

#[then(expr = "the transaction aggregate has {int} events")]
async fn then_transaction_has_events(world: &mut IntegrationWorld, expected: usize) {
    let transaction_id = world.transaction_id.expect("Transaction ID not set");
    let client = world.query_client.as_mut().expect("Query client not connected");

    let query = Query {
        domain: "transaction".to_string(),
        root: Some(IntegrationWorld::make_proto_uuid(transaction_id)),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };

    let mut stream = client
        .get_events(tonic::Request::new(query))
        .await
        .expect("Query failed")
        .into_inner();

    let mut total_events = 0;
    while let Some(book) = stream.message().await.expect("Stream error") {
        total_events += book.pages.len();
    }

    assert_eq!(
        total_events, expected,
        "Expected {} events, got {}",
        expected, total_events
    );
}

#[then(expr = "the latest event type is {string}")]
async fn then_latest_event_type(world: &mut IntegrationWorld, expected_type: String) {
    let response = world.last_response.as_ref().expect("No response");
    let book = response.books.last().expect("No event books in response");
    let page = book.pages.last().expect("No events in book");
    let event = page.event.as_ref().expect("No event data");

    assert!(
        event.type_url.contains(&expected_type),
        "Expected event type containing '{}', got '{}'",
        expected_type,
        event.type_url
    );
}

#[then(expr = "I receive {int} event")]
async fn then_receive_event(world: &mut IntegrationWorld, count: usize) {
    then_receive_events(world, count).await;
}

#[then(expr = "I receive {int} events")]
async fn then_receive_events(world: &mut IntegrationWorld, expected: usize) {
    assert_eq!(
        world.queried_events.len(),
        expected,
        "Expected {} events, got {}",
        expected,
        world.queried_events.len()
    );
}

#[then(expr = "the event at sequence {int} has type {string}")]
async fn then_event_at_sequence_has_type(
    world: &mut IntegrationWorld,
    sequence: usize,
    expected_type: String,
) {
    let event = world
        .queried_events
        .get(sequence)
        .expect(&format!("No event at sequence {}", sequence));

    let event_data = event.event.as_ref().expect("No event data");
    assert!(
        event_data.type_url.contains(&expected_type),
        "Expected event type containing '{}', got '{}'",
        expected_type,
        event_data.type_url
    );
}

#[then(expr = "a projection was returned from projector {string}")]
async fn then_projection_from_projector(world: &mut IntegrationWorld, projector_name: String) {
    let response = world.last_response.as_ref().expect("No response");

    let projection = response
        .projections
        .iter()
        .find(|p| p.projector == projector_name);

    assert!(
        projection.is_some(),
        "Expected projection from '{}', got projections: {:?}",
        projector_name,
        response.projections.iter().map(|p| &p.projector).collect::<Vec<_>>()
    );
}

#[then(expr = "the projection contains a Receipt with total {int} cents")]
async fn then_projection_contains_receipt(world: &mut IntegrationWorld, expected_total: i32) {
    let response = world.last_response.as_ref().expect("No response");

    let projection = response
        .projections
        .iter()
        .find(|p| p.projector == "receipt")
        .expect("No receipt projection found");

    let projection_data = projection
        .projection
        .as_ref()
        .expect("No projection data");

    assert!(
        projection_data.type_url.contains("Receipt"),
        "Expected Receipt type, got '{}'",
        projection_data.type_url
    );

    let receipt = Receipt::decode(projection_data.value.as_slice())
        .expect("Failed to decode Receipt");

    assert_eq!(
        receipt.final_total_cents, expected_total,
        "Expected total {} cents, got {} cents",
        expected_total, receipt.final_total_cents
    );

    println!("Receipt validated:");
    println!("  Transaction: {}", receipt.transaction_id);
    println!("  Total: ${:.2}", receipt.final_total_cents as f64 / 100.0);
    println!("  Items: {}", receipt.items.len());
}

#[tokio::main]
async fn main() {
    // SQLite requires sequential execution to avoid "database is locked" errors
    let use_sqlite = env::var("EVENTED_STORAGE_TYPE")
        .map(|v| v == "sqlite")
        .unwrap_or(true);  // Default to SQLite

    let cucumber = IntegrationWorld::cucumber();

    let cucumber = if use_sqlite {
        cucumber.max_concurrent_scenarios(1)
    } else {
        cucumber
    };

    cucumber.run("tests/integration/features").await;
}
