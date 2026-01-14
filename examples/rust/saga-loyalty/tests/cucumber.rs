//! Cucumber BDD tests for Loyalty Saga business logic.

use cucumber::{given, then, when, World};
use prost::Message;

use angzarr::proto::CommandBook;
use saga_loyalty::{AddLoyaltyPoints, LoyaltyPointsSaga, TransactionCompleted, TransactionCreated};

#[derive(Debug, Default, World)]
pub struct SagaWorld {
    saga: Option<LoyaltyPointsSaga>,
    event: Option<prost_types::Any>,
    result_commands: Vec<CommandBook>,
}

impl SagaWorld {
    fn saga(&self) -> &LoyaltyPointsSaga {
        self.saga.as_ref().expect("saga not initialized")
    }
}

// --- Given steps ---

#[given(expr = "a TransactionCreated event with customer {string} and subtotal {int}")]
fn transaction_created_event(world: &mut SagaWorld, customer_id: String, subtotal: i32) {
    world.saga = Some(LoyaltyPointsSaga::new());
    let event = TransactionCreated {
        customer_id,
        items: vec![],
        subtotal_cents: subtotal,
        created_at: None,
    };
    world.event = Some(prost_types::Any {
        type_url: "type.examples/examples.TransactionCreated".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a TransactionCompleted event with {int} loyalty points earned")]
fn transaction_completed_event(world: &mut SagaWorld, points: i32) {
    world.saga = Some(LoyaltyPointsSaga::new());
    let event = TransactionCompleted {
        final_total_cents: points * 100, // Reasonable default
        payment_method: "card".to_string(),
        loyalty_points_earned: points,
        completed_at: None,
    };
    world.event = Some(prost_types::Any {
        type_url: "type.examples/examples.TransactionCompleted".to_string(),
        value: event.encode_to_vec(),
    });
}

// --- When steps ---

#[when("I process the saga")]
fn process_saga(world: &mut SagaWorld) {
    let event = world.event.as_ref().expect("No event to process");
    world.result_commands = world.saga().process_event_public(event);
}

// --- Then steps ---

#[then("no commands are generated")]
fn no_commands_generated(world: &mut SagaWorld) {
    assert!(
        world.result_commands.is_empty(),
        "Expected no commands, got {}",
        world.result_commands.len()
    );
}

#[then("an AddLoyaltyPoints command is generated")]
fn add_loyalty_points_command_generated(world: &mut SagaWorld) {
    assert!(
        !world.result_commands.is_empty(),
        "Expected commands but got none"
    );
    let cmd_book = &world.result_commands[0];
    assert!(!cmd_book.pages.is_empty(), "Command book has no pages");
    let cmd = cmd_book.pages[0].command.as_ref().expect("No command");
    assert!(
        cmd.type_url.ends_with("AddLoyaltyPoints"),
        "Expected AddLoyaltyPoints, got {}",
        cmd.type_url
    );
}

#[then(expr = "the command has points {int}")]
fn command_has_points(world: &mut SagaWorld, points: i32) {
    let cmd_book = &world.result_commands[0];
    let cmd_any = cmd_book.pages[0].command.as_ref().expect("No command");
    let cmd = AddLoyaltyPoints::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(cmd.points, points, "Expected {} points, got {}", points, cmd.points);
}

#[then(expr = "the command has domain {string}")]
fn command_has_domain(world: &mut SagaWorld, domain: String) {
    let cmd_book = &world.result_commands[0];
    let cover = cmd_book.cover.as_ref().expect("No cover");
    assert_eq!(cover.domain, domain, "Expected domain '{}', got '{}'", domain, cover.domain);
}

#[then(expr = "the command reason contains {string}")]
fn command_reason_contains(world: &mut SagaWorld, substring: String) {
    let cmd_book = &world.result_commands[0];
    let cmd_any = cmd_book.pages[0].command.as_ref().expect("No command");
    let cmd = AddLoyaltyPoints::decode(cmd_any.value.as_slice()).expect("Failed to decode");
    assert!(
        cmd.reason.contains(&substring),
        "Expected reason to contain '{}', got '{}'",
        substring,
        cmd.reason
    );
}

fn main() {
    futures::executor::block_on(SagaWorld::run("tests/features"));
}
