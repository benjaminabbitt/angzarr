//! Cucumber BDD tests for Transaction business logic.

use cucumber::{gherkin::Step, given, then, when, World};
use prost::Message;

use angzarr::interfaces::business_client::BusinessError;
use angzarr::proto::{CommandBook, CommandPage, EventBook, EventPage};
use common::proto::{
    ApplyDiscount, CancelTransaction, CompleteTransaction, CreateTransaction, DiscountApplied,
    LineItem, TransactionCancelled, TransactionCompleted, TransactionCreated, TransactionState,
};
use transaction::TransactionLogic;

#[derive(Debug, Default, World)]
pub struct TransactionWorld {
    logic: Option<TransactionLogic>,
    prior_events: Vec<prost_types::Any>,
    result_event_book: Option<EventBook>,
    error: Option<BusinessError>,
    state: Option<TransactionState>,
}

impl TransactionWorld {
    fn logic(&self) -> &TransactionLogic {
        self.logic.as_ref().expect("logic not initialized")
    }

    fn build_event_book(&self) -> Option<EventBook> {
        if self.prior_events.is_empty() {
            return None;
        }
        Some(EventBook {
            cover: None,
            snapshot: None,
            pages: self
                .prior_events
                .iter()
                .map(|event| EventPage {
                    sequence: None,
                    event: Some(event.clone()),
                    created_at: None,
                    synchronous: false,
                })
                .collect(),
            correlation_id: String::new(),
            snapshot_state: None,
        })
    }

    fn next_seq(&self) -> u32 {
        self.prior_events.len() as u32
    }
}

/// Parse a data table from a step into LineItems.
fn parse_items_table(step: &Step) -> Vec<LineItem> {
    let table = step.table.as_ref().expect("Step should have a table");
    table
        .rows
        .iter()
        .skip(1) // Skip header row
        .map(|row| LineItem {
            product_id: row[0].clone(),
            name: row[1].clone(),
            quantity: row[2].parse().expect("quantity should be a number"),
            unit_price_cents: row[3].parse().expect("unit_price_cents should be a number"),
        })
        .collect()
}

// --- Given steps ---

#[given("no prior events for the aggregate")]
fn no_prior_events(world: &mut TransactionWorld) {
    world.logic = Some(TransactionLogic::new());
    world.prior_events.clear();
    world.result_event_book = None;
    world.error = None;
    world.state = None;
}

#[given(expr = "a TransactionCreated event with customer {string} and subtotal {int}")]
fn transaction_created_event(world: &mut TransactionWorld, customer_id: String, subtotal: i32) {
    if world.logic.is_none() {
        world.logic = Some(TransactionLogic::new());
    }
    let event = TransactionCreated {
        customer_id,
        items: vec![LineItem {
            product_id: "SKU-001".to_string(),
            name: "Item".to_string(),
            quantity: 1,
            unit_price_cents: subtotal,
        }],
        subtotal_cents: subtotal,
        created_at: None,
    };
    world.prior_events.push(prost_types::Any {
        type_url: "type.examples/examples.TransactionCreated".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given("a TransactionCompleted event")]
fn transaction_completed_event(world: &mut TransactionWorld) {
    if world.logic.is_none() {
        world.logic = Some(TransactionLogic::new());
    }
    let event = TransactionCompleted {
        final_total_cents: 0,
        payment_method: "card".to_string(),
        loyalty_points_earned: 0,
        completed_at: None,
    };
    world.prior_events.push(prost_types::Any {
        type_url: "type.examples/examples.TransactionCompleted".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a DiscountApplied event with {int} cents discount")]
fn discount_applied_event(world: &mut TransactionWorld, discount_cents: i32) {
    if world.logic.is_none() {
        world.logic = Some(TransactionLogic::new());
    }
    let event = DiscountApplied {
        discount_type: "fixed".to_string(),
        value: discount_cents,
        discount_cents,
        coupon_code: String::new(),
    };
    world.prior_events.push(prost_types::Any {
        type_url: "type.examples/examples.DiscountApplied".to_string(),
        value: event.encode_to_vec(),
    });
}

// --- When steps ---

#[when(expr = "I handle a CreateTransaction command with customer {string} and items:")]
fn handle_create_transaction_with_items(
    world: &mut TransactionWorld,
    step: &Step,
    customer_id: String,
) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));

    let items = parse_items_table(step);
    let cmd = CreateTransaction {
        customer_id,
        items,
    };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.CreateTransaction".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_create_transaction_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when(expr = "I handle a CreateTransaction command with customer {string} and no items")]
fn handle_create_transaction_no_items(world: &mut TransactionWorld, customer_id: String) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));

    let cmd = CreateTransaction {
        customer_id,
        items: vec![],
    };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.CreateTransaction".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_create_transaction_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when(expr = "I handle an ApplyDiscount command with type {string} and value {int}")]
fn handle_apply_discount(world: &mut TransactionWorld, discount_type: String, value: i32) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));
    let next_seq = world.next_seq();

    let cmd = ApplyDiscount {
        discount_type,
        value,
        coupon_code: String::new(),
    };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.ApplyDiscount".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_apply_discount_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when(expr = "I handle a CompleteTransaction command with payment method {string}")]
fn handle_complete_transaction(world: &mut TransactionWorld, payment_method: String) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));
    let next_seq = world.next_seq();

    let cmd = CompleteTransaction { payment_method };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.CompleteTransaction".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_complete_transaction_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when(expr = "I handle a CancelTransaction command with reason {string}")]
fn handle_cancel_transaction(world: &mut TransactionWorld, reason: String) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));
    let next_seq = world.next_seq();

    let cmd = CancelTransaction { reason };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.CancelTransaction".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_cancel_transaction_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when("I rebuild the transaction state")]
fn rebuild_transaction_state(world: &mut TransactionWorld) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));
}

// --- Then steps ---

#[then("the result is a TransactionCreated event")]
fn result_is_transaction_created(world: &mut TransactionWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("TransactionCreated"),
        "Expected TransactionCreated, got {}",
        event.type_url
    );
}

#[then("the result is a DiscountApplied event")]
fn result_is_discount_applied(world: &mut TransactionWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("DiscountApplied"),
        "Expected DiscountApplied, got {}",
        event.type_url
    );
}

#[then("the result is a TransactionCompleted event")]
fn result_is_transaction_completed(world: &mut TransactionWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("TransactionCompleted"),
        "Expected TransactionCompleted, got {}",
        event.type_url
    );
}

#[then("the result is a TransactionCancelled event")]
fn result_is_transaction_cancelled(world: &mut TransactionWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("TransactionCancelled"),
        "Expected TransactionCancelled, got {}",
        event.type_url
    );
}

#[then(expr = "the command fails with status {string}")]
fn command_fails_with_status(world: &mut TransactionWorld, _status: String) {
    assert!(
        world.error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the event has customer_id {string}")]
fn event_has_customer_id(world: &mut TransactionWorld, customer_id: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event = TransactionCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.customer_id, customer_id);
}

#[then(expr = "the event has subtotal_cents {int}")]
fn event_has_subtotal_cents(world: &mut TransactionWorld, subtotal_cents: i32) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event = TransactionCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.subtotal_cents, subtotal_cents);
}

#[then(expr = "the event has discount_cents {int}")]
fn event_has_discount_cents(world: &mut TransactionWorld, discount_cents: i32) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event = DiscountApplied::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.discount_cents, discount_cents);
}

#[then(expr = "the event has final_total_cents {int}")]
fn event_has_final_total_cents(world: &mut TransactionWorld, final_total_cents: i32) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event =
        TransactionCompleted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.final_total_cents, final_total_cents);
}

#[then(expr = "the event has payment_method {string}")]
fn event_has_payment_method(world: &mut TransactionWorld, payment_method: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event =
        TransactionCompleted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.payment_method, payment_method);
}

#[then(expr = "the event has loyalty_points_earned {int}")]
fn event_has_loyalty_points_earned(world: &mut TransactionWorld, loyalty_points_earned: i32) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event =
        TransactionCompleted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.loyalty_points_earned, loyalty_points_earned);
}

#[then(expr = "the event has reason {string}")]
fn event_has_reason(world: &mut TransactionWorld, reason: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event =
        TransactionCancelled::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.reason, reason);
}

#[then(expr = "the state has customer_id {string}")]
fn state_has_customer_id(world: &mut TransactionWorld, customer_id: String) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.customer_id, customer_id);
}

#[then(expr = "the state has subtotal_cents {int}")]
fn state_has_subtotal_cents(world: &mut TransactionWorld, subtotal_cents: i32) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.subtotal_cents, subtotal_cents);
}

#[then(expr = "the state has status {string}")]
fn state_has_status(world: &mut TransactionWorld, status: String) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.status, status);
}

fn main() {
    futures::executor::block_on(TransactionWorld::run("tests/features"));
}
