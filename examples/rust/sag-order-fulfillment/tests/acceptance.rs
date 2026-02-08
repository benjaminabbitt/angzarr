//! Acceptance tests for Fulfillment Saga.
//!
//! These tests verify the saga logic: converting OrderCompleted events to CreateShipment commands.

use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use angzarr::proto::{
    event_page::Sequence, CommandBook, Cover, EventBook, EventPage, Uuid as ProtoUuid,
};
use common::proto::{CreateShipment, OrderCancelled, OrderCompleted, OrderCreated};
use common::SagaLogic;
use sag_order_fulfillment::OrderFulfillmentSaga;

#[derive(Debug, Default, World)]
pub struct SagaWorld {
    current_event: Option<prost_types::Any>,
    current_root: Option<ProtoUuid>,
    current_root_string: String,
    current_correlation_id: String,
    generated_commands: Vec<CommandBook>,
}

impl SagaWorld {
    fn saga(&self) -> OrderFulfillmentSaga {
        OrderFulfillmentSaga::new()
    }

    fn set_root(&mut self, alias: &str) {
        let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, alias.as_bytes());
        self.current_root = Some(ProtoUuid {
            value: uuid.as_bytes().to_vec(),
        });
        self.current_root_string = uuid.to_string();
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
        customer_root: vec![],
        cart_root: vec![],
        items: vec![],
        fraud_check_result: "approved".to_string(),
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCompleted".to_string(),
        value: event.encode_to_vec(),
    });

    world.set_root(&order_id);
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
        customer_root: vec![],
        cart_root: vec![],
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCreated".to_string(),
        value: event.encode_to_vec(),
    });

    world.set_root(&order_id);
    world.current_correlation_id = Uuid::new_v4().to_string();
    world.generated_commands.clear();
}

#[given(expr = "an OrderCancelled event for order {string}")]
async fn order_cancelled_event(world: &mut SagaWorld, order_id: String) {
    let event = OrderCancelled {
        reason: "Test cancellation".to_string(),
        loyalty_points_used: 0,
        cancelled_at: None,
        customer_root: vec![],
        cart_root: vec![],
        items: vec![],
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCancelled".to_string(),
        value: event.encode_to_vec(),
    });

    world.set_root(&order_id);
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

    // Build EventBook from the current event
    let event_book = EventBook {
        cover: Some(Cover {
            domain: "order".to_string(),
            root: world.current_root.clone(),
            correlation_id: world.current_correlation_id.clone(),
            edition: None,
        }),
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(1)),
            created_at: None,
            event: Some(event.clone()),
        }],
        snapshot: None,
    };

    world.generated_commands = saga.execute(&event_book, &[]);
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

#[then("the command references the source order")]
async fn command_references_source_order(world: &mut SagaWorld) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    let cmd_any = cmd.pages[0].command.as_ref().expect("No command");
    let shipment = CreateShipment::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(
        shipment.order_id, world.current_root_string,
        "Command order_id should match source root UUID"
    );
}

#[then(expr = "the command has correlation_id {string}")]
async fn command_has_correlation_id(world: &mut SagaWorld, correlation_id: String) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    let cmd_correlation_id = cmd
        .cover
        .as_ref()
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");
    assert_eq!(cmd_correlation_id, correlation_id);
}

#[tokio::main]
async fn main() {
    SagaWorld::cucumber()
        .run("../../features/saga_fulfillment.feature")
        .await;
}
