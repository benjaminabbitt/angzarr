//! Cucumber BDD tests for Receipt Projector.

use cucumber::{gherkin::Step, given, then, when, World};
use prost::Message;

use projector_receipt::{
    DiscountApplied, LineItem, Receipt, ReceiptProjector, TransactionCompleted,
    TransactionCreated,
};

#[derive(Debug, Default, World)]
pub struct ReceiptProjectorWorld {
    projector: Option<ReceiptProjector>,
    events: Vec<prost_types::Any>,
    result: Option<Receipt>,
}

impl ReceiptProjectorWorld {
    fn projector(&self) -> &ReceiptProjector {
        self.projector.as_ref().expect("projector not initialized")
    }
}

// --- Given steps ---

#[given(expr = "a TransactionCreated event with customer {string} and subtotal {int}")]
fn transaction_created_simple(world: &mut ReceiptProjectorWorld, customer: String, subtotal: i32) {
    world.projector = Some(ReceiptProjector::new());
    let event = TransactionCreated {
        customer_id: customer,
        items: vec![],
        subtotal_cents: subtotal,
        created_at: None,
    };
    world.events.push(prost_types::Any {
        type_url: "type.examples/examples.TransactionCreated".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a TransactionCreated event with customer {string} and items:")]
fn transaction_created_with_items(world: &mut ReceiptProjectorWorld, customer: String, step: &Step) {
    world.projector = Some(ReceiptProjector::new());

    let mut items = Vec::new();
    let mut subtotal = 0;

    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            // Skip header row
            let product_id = row.get(0).map(|s| s.as_str()).unwrap_or("").to_string();
            let name = row.get(1).map(|s| s.as_str()).unwrap_or("").to_string();
            let quantity: i32 = row
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let unit_price_cents: i32 = row
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            subtotal += quantity * unit_price_cents;

            items.push(LineItem {
                product_id,
                name,
                quantity,
                unit_price_cents,
            });
        }
    }

    let event = TransactionCreated {
        customer_id: customer,
        items,
        subtotal_cents: subtotal,
        created_at: None,
    };

    world.events.push(prost_types::Any {
        type_url: "type.examples/examples.TransactionCreated".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a TransactionCompleted event with total {int} and payment {string}")]
fn transaction_completed(world: &mut ReceiptProjectorWorld, total: i32, payment: String) {
    if world.projector.is_none() {
        world.projector = Some(ReceiptProjector::new());
    }
    let event = TransactionCompleted {
        final_total_cents: total,
        payment_method: payment,
        loyalty_points_earned: 0,
        completed_at: None,
    };
    world.events.push(prost_types::Any {
        type_url: "type.examples/examples.TransactionCompleted".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a TransactionCompleted event with total {int} and payment {string} earning {int} points")]
fn transaction_completed_with_points(
    world: &mut ReceiptProjectorWorld,
    total: i32,
    payment: String,
    points: i32,
) {
    if world.projector.is_none() {
        world.projector = Some(ReceiptProjector::new());
    }
    let event = TransactionCompleted {
        final_total_cents: total,
        payment_method: payment,
        loyalty_points_earned: points,
        completed_at: None,
    };
    world.events.push(prost_types::Any {
        type_url: "type.examples/examples.TransactionCompleted".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a DiscountApplied event with {int} cents discount")]
fn discount_applied(world: &mut ReceiptProjectorWorld, discount_cents: i32) {
    if world.projector.is_none() {
        world.projector = Some(ReceiptProjector::new());
    }
    let event = DiscountApplied {
        discount_type: "fixed".to_string(),
        value: discount_cents,
        discount_cents,
        coupon_code: String::new(),
    };
    world.events.push(prost_types::Any {
        type_url: "type.examples/examples.DiscountApplied".to_string(),
        value: event.encode_to_vec(),
    });
}

// --- When steps ---

#[when("I project the events")]
fn project_events(world: &mut ReceiptProjectorWorld) {
    world.result = world.projector().project_events(&world.events);
}

// --- Then steps ---

#[then("no projection is generated")]
fn no_projection_generated(world: &mut ReceiptProjectorWorld) {
    assert!(
        world.result.is_none(),
        "Expected no projection but got one"
    );
}

#[then("a Receipt projection is generated")]
fn receipt_projection_generated(world: &mut ReceiptProjectorWorld) {
    assert!(
        world.result.is_some(),
        "Expected Receipt projection but got none"
    );
}

#[then(expr = "the receipt has customer_id {string}")]
fn receipt_has_customer_id(world: &mut ReceiptProjectorWorld, customer_id: String) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert_eq!(receipt.customer_id, customer_id);
}

#[then(expr = "the receipt has final_total_cents {int}")]
fn receipt_has_final_total(world: &mut ReceiptProjectorWorld, total: i32) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert_eq!(receipt.final_total_cents, total);
}

#[then(expr = "the receipt has payment_method {string}")]
fn receipt_has_payment_method(world: &mut ReceiptProjectorWorld, payment: String) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert_eq!(receipt.payment_method, payment);
}

#[then(expr = "the receipt has subtotal_cents {int}")]
fn receipt_has_subtotal(world: &mut ReceiptProjectorWorld, subtotal: i32) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert_eq!(receipt.subtotal_cents, subtotal);
}

#[then(expr = "the receipt has discount_cents {int}")]
fn receipt_has_discount(world: &mut ReceiptProjectorWorld, discount: i32) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert_eq!(receipt.discount_cents, discount);
}

#[then(expr = "the receipt has loyalty_points_earned {int}")]
fn receipt_has_loyalty_points(world: &mut ReceiptProjectorWorld, points: i32) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert_eq!(receipt.loyalty_points_earned, points);
}

#[then(expr = "the receipt formatted_text contains {string}")]
fn receipt_formatted_text_contains(world: &mut ReceiptProjectorWorld, text: String) {
    let receipt = world.result.as_ref().expect("No receipt");
    assert!(
        receipt.formatted_text.contains(&text),
        "Expected formatted_text to contain '{}', got: {}",
        text,
        receipt.formatted_text
    );
}

fn main() {
    futures::executor::block_on(ReceiptProjectorWorld::run("tests/features"));
}
