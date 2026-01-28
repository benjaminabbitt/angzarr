//! Acceptance tests for Order Cancellation Saga.
//!
//! These tests verify the saga logic: generating compensation commands when orders are cancelled.

use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use angzarr::proto::{
    event_page::Sequence, CommandBook, Cover, EventBook, EventPage, Uuid as ProtoUuid,
};
use common::proto::{OrderCancelled, OrderCreated, ReleaseReservation};
use saga_cancellation::{CancellationSaga, SOURCE_DOMAIN};

#[derive(Debug, Default, World)]
pub struct SagaWorld {
    current_event: Option<prost_types::Any>,
    current_root: Option<ProtoUuid>,
    current_correlation_id: String,
    generated_commands: Vec<CommandBook>,
}

impl SagaWorld {
    fn saga(&self) -> CancellationSaga {
        CancellationSaga::new()
    }
}

// =============================================================================
// Given Steps
// =============================================================================

#[given(expr = "an OrderCancelled event for order {string} with reason {string}")]
async fn order_cancelled_with_reason(world: &mut SagaWorld, order_id: String, reason: String) {
    let event = OrderCancelled {
        reason,
        loyalty_points_used: 0,
        cancelled_at: None,
        customer_root: vec![],
        items: vec![],
        cart_root: vec![],
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

#[given(expr = "an OrderCancelled event for order {string} with loyalty_points_used {int}")]
async fn order_cancelled_with_points(world: &mut SagaWorld, order_id: String, points: i32) {
    let event = OrderCancelled {
        reason: "Test cancellation".to_string(),
        loyalty_points_used: points,
        cancelled_at: None,
        customer_root: vec![],
        items: vec![],
        cart_root: vec![],
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

#[when("I process the cancellation saga")]
async fn process_saga(world: &mut SagaWorld) {
    let saga = world.saga();
    let event = world.current_event.as_ref().expect("No event set");

    // Build EventBook from the current event
    let event_book = EventBook {
        cover: Some(Cover {
            domain: SOURCE_DOMAIN.to_string(),
            root: world.current_root.clone(),
            correlation_id: world.current_correlation_id.clone(),
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

#[then("a ReleaseReservation command is generated")]
async fn release_reservation_generated(world: &mut SagaWorld) {
    assert!(
        !world.generated_commands.is_empty(),
        "Expected at least one command to be generated"
    );

    let has_release = world.generated_commands.iter().any(|cmd| {
        cmd.pages
            .first()
            .and_then(|p| p.command.as_ref())
            .map(|c| c.type_url.contains("ReleaseReservation"))
            .unwrap_or(false)
    });

    assert!(has_release, "Expected a ReleaseReservation command");
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
    let release = ReleaseReservation::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(release.order_id, order_id);
}

#[then(expr = "commands are generated for {string} and {string} domains")]
async fn commands_for_multiple_domains(world: &mut SagaWorld, domain1: String, domain2: String) {
    assert!(
        world.generated_commands.len() >= 2,
        "Expected at least 2 commands, got {}",
        world.generated_commands.len()
    );

    let domains: Vec<String> = world
        .generated_commands
        .iter()
        .filter_map(|cmd| cmd.cover.as_ref().map(|c| c.domain.clone()))
        .collect();

    assert!(
        domains.contains(&domain1),
        "Expected command for domain '{}', found {:?}",
        domain1,
        domains
    );
    assert!(
        domains.contains(&domain2),
        "Expected command for domain '{}', found {:?}",
        domain2,
        domains
    );
}

#[then("only an inventory command is generated")]
async fn only_inventory_command(world: &mut SagaWorld) {
    assert_eq!(
        world.generated_commands.len(),
        1,
        "Expected exactly 1 command, got {}",
        world.generated_commands.len()
    );

    let cmd = &world.generated_commands[0];
    let cover = cmd.cover.as_ref().expect("No cover");
    assert_eq!(cover.domain, "inventory");
}

#[then("no commands are generated")]
async fn no_commands_generated(world: &mut SagaWorld) {
    assert!(
        world.generated_commands.is_empty(),
        "Expected no commands, but {} were generated",
        world.generated_commands.len()
    );
}

#[then(expr = "all commands have correlation_id {string}")]
async fn all_commands_have_correlation_id(world: &mut SagaWorld, correlation_id: String) {
    assert!(
        !world.generated_commands.is_empty(),
        "No commands generated"
    );

    for cmd in &world.generated_commands {
        let cmd_correlation_id = cmd
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            cmd_correlation_id, correlation_id,
            "Command has correlation_id '{}', expected '{}'",
            cmd_correlation_id, correlation_id
        );
    }
}

#[tokio::main]
async fn main() {
    SagaWorld::cucumber()
        .run("tests/features/saga_cancellation.feature")
        .await;
}
