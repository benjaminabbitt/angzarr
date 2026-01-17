//! Acceptance tests for Fulfillment Saga.
//!
//! These tests verify the saga logic: converting OrderCompleted events to CreateShipment commands.

use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use angzarr::proto::{CommandBook, Uuid as ProtoUuid};
use common::proto::{CreateShipment, OrderCancelled, OrderCompleted, OrderCreated};
use saga_fulfillment::FulfillmentSaga;

#[derive(Debug, Default, World)]
pub struct SagaWorld {
    current_event: Option<prost_types::Any>,
    current_root: Option<ProtoUuid>,
    current_correlation_id: String,
    generated_commands: Vec<CommandBook>,
}

impl SagaWorld {
    fn saga(&self) -> FulfillmentSaga {
        FulfillmentSaga::new()
    }
}

// =============================================================================
// Given Steps
// =============================================================================

#[given(expr = "an OrderCompleted event for order {string}")]
async fn order_completed_event(world: &mut SagaWorld, order_id: String) {
    let event = OrderCompleted {
        final_total_cents: 5000,
        payment_method: "card".to_string(),
        payment_reference: "PAY-TEST".to_string(),
        loyalty_points_earned: 50,
        completed_at: None,
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCompleted".to_string(),
        value: event.encode_to_vec(),
    });

    // Use order_id as root
    world.current_root = Some(ProtoUuid {
        value: order_id.as_bytes().to_vec(),
    });
    world.current_correlation_id = Uuid::new_v4().to_string();
    world.generated_commands.clear();
}

#[given(expr = "an OrderCreated event for order {string}")]
async fn order_created_event(world: &mut SagaWorld, order_id: String) {
    let event = OrderCreated {
        customer_id: "CUST-TEST".to_string(),
        items: Vec::new(),
        subtotal_cents: 0,
        created_at: None,
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCreated".to_string(),
        value: event.encode_to_vec(),
    });

    world.current_root = Some(ProtoUuid {
        value: order_id.as_bytes().to_vec(),
    });
    world.current_correlation_id = Uuid::new_v4().to_string();
    world.generated_commands.clear();
}

#[given(expr = "an OrderCancelled event for order {string}")]
async fn order_cancelled_event(world: &mut SagaWorld, order_id: String) {
    let event = OrderCancelled {
        reason: "Test cancellation".to_string(),
        loyalty_points_used: 0,
        cancelled_at: None,
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCancelled".to_string(),
        value: event.encode_to_vec(),
    });

    world.current_root = Some(ProtoUuid {
        value: order_id.as_bytes().to_vec(),
    });
    world.current_correlation_id = Uuid::new_v4().to_string();
    world.generated_commands.clear();
}

#[given(expr = "the correlation_id is {string}")]
async fn set_correlation_id(world: &mut SagaWorld, correlation_id: String) {
    world.current_correlation_id = correlation_id;
}

// =============================================================================
// When Steps
// =============================================================================

#[when("I process the fulfillment saga")]
async fn process_saga(world: &mut SagaWorld) {
    let saga = world.saga();
    let event = world.current_event.as_ref().expect("No event set");
    let root = world.current_root.as_ref();

    if let Some(cmd) = saga.process_event_public(event, root, &world.current_correlation_id) {
        world.generated_commands.push(cmd);
    }
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("a CreateShipment command is generated")]
async fn create_shipment_generated(world: &mut SagaWorld) {
    assert!(
        !world.generated_commands.is_empty(),
        "Expected a command to be generated"
    );

    let cmd = &world.generated_commands[0];
    let cmd_any = cmd.pages[0].command.as_ref().expect("No command in page");
    assert!(
        cmd_any.type_url.contains("CreateShipment"),
        "Expected CreateShipment command, got {}",
        cmd_any.type_url
    );
}

#[then("no commands are generated")]
async fn no_commands_generated(world: &mut SagaWorld) {
    assert!(
        world.generated_commands.is_empty(),
        "Expected no commands, but {} were generated",
        world.generated_commands.len()
    );
}

#[then(expr = "the command targets {string} domain")]
async fn command_targets_domain(world: &mut SagaWorld, domain: String) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    let cover = cmd.cover.as_ref().expect("No cover");
    assert_eq!(cover.domain, domain);
}

#[then(expr = "the command has order_id {string}")]
async fn command_has_order_id(world: &mut SagaWorld, order_id: String) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    let cmd_any = cmd.pages[0].command.as_ref().expect("No command");
    let shipment = CreateShipment::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(shipment.order_id, order_id);
}

#[then(expr = "the command has correlation_id {string}")]
async fn command_has_correlation_id(world: &mut SagaWorld, correlation_id: String) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    assert_eq!(cmd.correlation_id, correlation_id);
}

#[tokio::main]
async fn main() {
    SagaWorld::cucumber()
        .run("tests/features/saga_fulfillment.feature")
        .await;
}
