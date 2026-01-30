//! Acceptance tests for Loyalty Earn Saga.
//!
//! These tests verify the saga logic: converting OrderCompleted events to AddLoyaltyPoints commands.

use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use angzarr::proto::{
    event_page::Sequence, CommandBook, Cover, EventBook, EventPage, Uuid as ProtoUuid,
};
use common::proto::{AddLoyaltyPoints, OrderCompleted, OrderCreated};
use saga_loyalty_earn::{LoyaltyEarnSaga, SOURCE_DOMAIN};

#[derive(Debug, Default, World)]
pub struct SagaWorld {
    current_event: Option<prost_types::Any>,
    current_root: Option<ProtoUuid>,
    current_correlation_id: String,
    generated_commands: Vec<CommandBook>,
}

impl SagaWorld {
    fn saga(&self) -> LoyaltyEarnSaga {
        LoyaltyEarnSaga::new()
    }
}

// =============================================================================
// Given Steps
// =============================================================================

#[given(expr = "an OrderCompleted event with loyalty_points_earned {int} for customer {string}")]
async fn order_completed_event(
    world: &mut SagaWorld,
    loyalty_points_earned: i32,
    customer_id: String,
) {
    // Derive aggregate root from customer_id
    let root_uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, customer_id.as_bytes());

    let event = OrderCompleted {
        final_total_cents: loyalty_points_earned * 100,
        payment_method: "card".to_string(),
        payment_reference: "PAY-TEST".to_string(),
        loyalty_points_earned,
        completed_at: None,
        customer_root: root_uuid.as_bytes().to_vec(),
        cart_root: vec![],
        items: vec![],
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCompleted".to_string(),
        value: event.encode_to_vec(),
    });

    world.current_root = Some(ProtoUuid {
        value: root_uuid.as_bytes().to_vec(),
    });
    world.current_correlation_id = Uuid::new_v4().to_string();
    world.generated_commands.clear();
}

#[given(expr = "an OrderCreated event for customer {string}")]
async fn order_created_event(world: &mut SagaWorld, customer_id: String) {
    let root_uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, customer_id.as_bytes());

    let event = OrderCreated {
        customer_id: customer_id.clone(),
        items: Vec::new(),
        subtotal_cents: 0,
        created_at: None,
        customer_root: root_uuid.as_bytes().to_vec(),
        cart_root: vec![],
    };

    world.current_event = Some(prost_types::Any {
        type_url: "type.examples/examples.OrderCreated".to_string(),
        value: event.encode_to_vec(),
    });

    world.current_root = Some(ProtoUuid {
        value: root_uuid.as_bytes().to_vec(),
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

#[when("I process the loyalty earn saga")]
async fn process_saga(world: &mut SagaWorld) {
    let saga = world.saga();
    let event = world.current_event.as_ref().expect("No event set");

    // Build EventBook from the current event
    let event_book = EventBook {
        cover: Some(Cover {
            domain: SOURCE_DOMAIN.to_string(),
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
        snapshot_state: None,
    };

    // Use the public handle() interface
    world.generated_commands = saga.handle(&event_book);
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("an AddLoyaltyPoints command is generated")]
async fn add_loyalty_points_generated(world: &mut SagaWorld) {
    assert!(
        !world.generated_commands.is_empty(),
        "Expected a command to be generated"
    );

    let cmd = &world.generated_commands[0];
    let cmd_any = cmd.pages[0].command.as_ref().expect("No command in page");
    assert!(
        cmd_any.type_url.contains("AddLoyaltyPoints"),
        "Expected AddLoyaltyPoints command, got {}",
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

#[then(expr = "the command has points {int}")]
async fn command_has_points(world: &mut SagaWorld, points: i32) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    let cmd_any = cmd.pages[0].command.as_ref().expect("No command");
    let add_points = AddLoyaltyPoints::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(add_points.points, points);
}

#[then(expr = "the command has reason containing {string}")]
async fn command_has_reason_containing(world: &mut SagaWorld, substring: String) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );
    let cmd = &world.generated_commands[0];
    let cmd_any = cmd.pages[0].command.as_ref().expect("No command");
    let add_points = AddLoyaltyPoints::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert!(
        add_points
            .reason
            .to_lowercase()
            .contains(&substring.to_lowercase()),
        "Expected '{}' in reason '{}'",
        substring,
        add_points.reason
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
        .run("../../features/saga_loyalty_earn.feature")
        .await;
}
