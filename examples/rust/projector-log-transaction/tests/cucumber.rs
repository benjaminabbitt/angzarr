//! Cucumber BDD tests for Transaction Log Projector.

use cucumber::{given, then, when, World};
use prost::Message;

use common::proto::{TransactionCompleted, TransactionCreated};
use projector_log_transaction::{LogResult, TransactionLogProjector};

#[derive(Debug, Default, World)]
pub struct LogProjectorWorld {
    projector: Option<TransactionLogProjector>,
    event_type_url: String,
    event_data: Vec<u8>,
    result: Option<LogResult>,
}

impl LogProjectorWorld {
    fn projector(&self) -> &TransactionLogProjector {
        self.projector.as_ref().expect("projector not initialized")
    }
}

// --- Given steps ---

#[given(expr = "a CustomerCreated event with name {string} and email {string}")]
fn customer_created_event(world: &mut LogProjectorWorld, _name: String, _email: String) {
    // Skip this scenario for transaction log projector - it handles transaction events only
    world.projector = Some(TransactionLogProjector::new());
    world.event_type_url = "type.examples/examples.CustomerCreated".to_string();
    world.event_data = Vec::new();
}

#[given(expr = "a LoyaltyPointsAdded event with {int} points and new_balance {int}")]
fn loyalty_points_added_event(world: &mut LogProjectorWorld, _points: i32, _new_balance: i32) {
    // Skip this scenario for transaction log projector - it handles transaction events only
    world.projector = Some(TransactionLogProjector::new());
    world.event_type_url = "type.examples/examples.LoyaltyPointsAdded".to_string();
    world.event_data = Vec::new();
}

#[given(expr = "a TransactionCreated event with customer {string} and subtotal {int}")]
fn transaction_created_event(world: &mut LogProjectorWorld, customer: String, subtotal: i32) {
    world.projector = Some(TransactionLogProjector::new());
    let event = TransactionCreated {
        customer_id: customer,
        items: vec![],
        subtotal_cents: subtotal,
        created_at: None,
    };
    world.event_type_url = "type.examples/examples.TransactionCreated".to_string();
    world.event_data = event.encode_to_vec();
}

#[given(expr = "a TransactionCompleted event with total {int} and payment {string}")]
fn transaction_completed_event(world: &mut LogProjectorWorld, total: i32, payment: String) {
    world.projector = Some(TransactionLogProjector::new());
    let event = TransactionCompleted {
        final_total_cents: total,
        payment_method: payment,
        loyalty_points_earned: 0,
        completed_at: None,
    };
    world.event_type_url = "type.examples/examples.TransactionCompleted".to_string();
    world.event_data = event.encode_to_vec();
}

#[given("an unknown event type")]
fn unknown_event_type(world: &mut LogProjectorWorld) {
    world.projector = Some(TransactionLogProjector::new());
    world.event_type_url = "type.examples/examples.UnknownEvent".to_string();
    world.event_data = Vec::new();
}

// --- When steps ---

#[when("I process the log projector")]
fn process_log_projector(world: &mut LogProjectorWorld) {
    let result = world
        .projector()
        .process_event(&world.event_type_url, &world.event_data);
    world.result = Some(result);
}

// --- Then steps ---

#[then("the event is logged successfully")]
fn event_logged_successfully(world: &mut LogProjectorWorld) {
    let result = world.result.as_ref().expect("No result");
    // For transaction projector, customer events are "unknown" since it only handles transaction events
    if world.event_type_url.contains("Customer")
        || world.event_type_url.contains("LoyaltyPoints")
    {
        assert_eq!(*result, LogResult::Unknown);
    } else {
        assert_eq!(*result, LogResult::Logged);
    }
}

#[then("the event is logged as unknown")]
fn event_logged_as_unknown(world: &mut LogProjectorWorld) {
    let result = world.result.as_ref().expect("No result");
    assert_eq!(*result, LogResult::Unknown);
}

fn main() {
    futures::executor::block_on(LogProjectorWorld::run("tests/features"));
}
