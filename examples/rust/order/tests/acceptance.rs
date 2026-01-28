//! Acceptance tests for Order domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p order --test acceptance

use angzarr::proto::CommandResponse;
use angzarr_client::{type_name_from_url, Client, ClientError, CommandBuilderExt, QueryBuilderExt};
use cucumber::{gherkin::Step, given, then, when, World};
use prost::Message;
use uuid::Uuid;

use common::proto::{
    ApplyLoyaltyDiscount, CancelOrder, ConfirmPayment, CreateOrder, LineItem,
    LoyaltyDiscountApplied, OrderCancelled, OrderCompleted, OrderCreated, PaymentSubmitted,
    SubmitPayment,
};

/// Default Angzarr port - standard across all languages/containers
const DEFAULT_ANGZARR_PORT: u16 = 1350;

/// Get gateway endpoint from environment or default
fn get_gateway_endpoint() -> String {
    if let Ok(endpoint) = std::env::var("ANGZARR_ENDPOINT") {
        return endpoint;
    }
    let host = std::env::var("ANGZARR_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = std::env::var("ANGZARR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_ANGZARR_PORT);
    format!("http://{}:{}", host, port)
}

#[derive(World)]
#[world(init = Self::new)]
pub struct OrderAcceptanceWorld {
    client: Option<Client>,
    current_order_id: Option<Uuid>,
    current_sequence: u32,
    current_subtotal: i32,
    current_discount: i32,
    last_response: Option<CommandResponse>,
    last_error: Option<String>,
}

impl std::fmt::Debug for OrderAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrderAcceptanceWorld")
            .field("current_order_id", &self.current_order_id)
            .finish()
    }
}

impl OrderAcceptanceWorld {
    async fn new() -> Self {
        Self {
            client: None,
            current_order_id: None,
            current_sequence: 0,
            current_subtotal: 0,
            current_discount: 0,
            last_response: None,
            last_error: None,
        }
    }

    async fn client(&mut self) -> &Client {
        if self.client.is_none() {
            let endpoint = get_gateway_endpoint();
            self.client = Some(
                Client::connect(&endpoint)
                    .await
                    .expect("Failed to connect to gateway"),
            );
        }
        self.client.as_ref().unwrap()
    }

    fn order_root(&self) -> Uuid {
        self.current_order_id.expect("No order ID set")
    }

    async fn execute_command<M: Message>(
        &mut self,
        command: M,
        type_url: &str,
    ) -> Result<CommandResponse, ClientError> {
        let order_id = self.order_root();
        let sequence = self.current_sequence;
        let client = self.client().await;

        client
            .gateway
            .command("order", order_id)
            .with_sequence(sequence)
            .with_command(format!("type.googleapis.com/{}", type_url), &command)
            .execute()
            .await
    }

    fn handle_result(&mut self, result: Result<CommandResponse, ClientError>) {
        match result {
            Ok(response) => {
                self.last_response = Some(response);
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(e.message());
                self.last_response = None;
            }
        }
    }

    fn extract_event_type(event: &prost_types::Any) -> String {
        type_name_from_url(&event.type_url).to_string()
    }

    fn parse_items_from_table(step: &Step) -> Vec<LineItem> {
        let mut items = Vec::new();
        if let Some(table) = &step.table {
            // Skip header row
            for row in table.rows.iter().skip(1) {
                if row.len() >= 4 {
                    // Generate deterministic product_root from product_id
                    let product_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, row[0].as_bytes())
                        .as_bytes()
                        .to_vec();
                    items.push(LineItem {
                        product_id: row[0].clone(),
                        name: row[1].clone(),
                        quantity: row[2].parse().unwrap_or(0),
                        unit_price_cents: row[3].parse().unwrap_or(0),
                        product_root,
                    });
                }
            }
        }
        items
    }
}

// =============================================================================
// Given Steps
// =============================================================================

#[given("no prior events for the order aggregate")]
async fn no_prior_events(world: &mut OrderAcceptanceWorld) {
    world.current_order_id = Some(Uuid::new_v4());
    world.current_sequence = 0;
    world.current_subtotal = 0;
    world.current_discount = 0;
    world.last_response = None;
    world.last_error = None;
}

#[given(expr = "an OrderCreated event with customer_id {string} and subtotal {int}")]
async fn order_created_event(world: &mut OrderAcceptanceWorld, customer_id: String, subtotal: i32) {
    if world.current_order_id.is_none() {
        world.current_order_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }
    world.current_subtotal = subtotal;

    // Generate deterministic roots from IDs
    let customer_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, customer_id.as_bytes())
        .as_bytes()
        .to_vec();
    let cart_root = Uuid::new_v4().as_bytes().to_vec();
    let product_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"SETUP-ITEM")
        .as_bytes()
        .to_vec();

    // Create a simple order with one item that totals to the subtotal
    let command = CreateOrder {
        customer_id,
        items: vec![LineItem {
            product_id: "SETUP-ITEM".to_string(),
            name: "Setup Item".to_string(),
            quantity: 1,
            unit_price_cents: subtotal,
            product_root,
        }],
        customer_root,
        cart_root,
    };
    let result = world.execute_command(command, "examples.CreateOrder").await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(e) => {
            panic!("Given step failed: OrderCreated - {}", e.message());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(expr = "a LoyaltyDiscountApplied event with {int} points")]
async fn loyalty_discount_applied_event(world: &mut OrderAcceptanceWorld, points: i32) {
    world.current_discount = points; // 1 point = 1 cent discount

    let command = ApplyLoyaltyDiscount {
        points,
        discount_cents: points,
    };
    let result = world
        .execute_command(command, "examples.ApplyLoyaltyDiscount")
        .await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!(
            "Given step failed: LoyaltyDiscountApplied - {}",
            e.message()
        ),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("a PaymentSubmitted event")]
async fn payment_submitted_event(world: &mut OrderAcceptanceWorld) {
    let amount = world.current_subtotal - world.current_discount;
    let command = SubmitPayment {
        payment_method: "card".to_string(),
        amount_cents: amount,
    };
    let result = world
        .execute_command(command, "examples.SubmitPayment")
        .await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: PaymentSubmitted - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("an OrderCompleted event")]
async fn order_completed_event(world: &mut OrderAcceptanceWorld) {
    let command = ConfirmPayment {
        payment_reference: "SETUP-PAY-REF".to_string(),
    };
    let result = world
        .execute_command(command, "examples.ConfirmPayment")
        .await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: OrderCompleted - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("an OrderCancelled event")]
async fn order_cancelled_event(world: &mut OrderAcceptanceWorld) {
    let command = CancelOrder {
        reason: "Setup cancellation".to_string(),
    };
    let result = world.execute_command(command, "examples.CancelOrder").await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: OrderCancelled - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

// =============================================================================
// When Steps
// =============================================================================

#[when(expr = "I handle a CreateOrder command with customer_id {string} and items:")]
async fn handle_create_order_with_items(
    world: &mut OrderAcceptanceWorld,
    step: &Step,
    customer_id: String,
) {
    if world.current_order_id.is_none() {
        world.current_order_id = Some(Uuid::new_v4());
    }

    let items = OrderAcceptanceWorld::parse_items_from_table(step);
    world.current_subtotal = items.iter().map(|i| i.quantity * i.unit_price_cents).sum();

    // Generate deterministic roots from IDs
    let customer_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, customer_id.as_bytes())
        .as_bytes()
        .to_vec();
    let cart_root = Uuid::new_v4().as_bytes().to_vec();

    let command = CreateOrder {
        customer_id,
        items,
        customer_root,
        cart_root,
    };
    let result = world.execute_command(command, "examples.CreateOrder").await;
    world.handle_result(result);
}

#[when(expr = "I handle a CreateOrder command with customer_id {string} and no items")]
async fn handle_create_order_no_items(world: &mut OrderAcceptanceWorld, customer_id: String) {
    if world.current_order_id.is_none() {
        world.current_order_id = Some(Uuid::new_v4());
    }

    // Generate deterministic roots from IDs
    let customer_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, customer_id.as_bytes())
        .as_bytes()
        .to_vec();
    let cart_root = Uuid::new_v4().as_bytes().to_vec();

    let command = CreateOrder {
        customer_id,
        items: vec![],
        customer_root,
        cart_root,
    };
    let result = world.execute_command(command, "examples.CreateOrder").await;
    world.handle_result(result);
}

#[when(expr = "I handle an ApplyLoyaltyDiscount command with points {int} worth {int} cents")]
async fn handle_apply_loyalty_discount(
    world: &mut OrderAcceptanceWorld,
    points: i32,
    discount_cents: i32,
) {
    let command = ApplyLoyaltyDiscount {
        points,
        discount_cents,
    };
    let result = world
        .execute_command(command, "examples.ApplyLoyaltyDiscount")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_discount = discount_cents;
        }
        Err(e) => {
            world.last_error = Some(e.message());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a SubmitPayment command with method {string} and amount {int} cents")]
async fn handle_submit_payment(
    world: &mut OrderAcceptanceWorld,
    payment_method: String,
    amount_cents: i32,
) {
    let command = SubmitPayment {
        payment_method,
        amount_cents,
    };
    let result = world
        .execute_command(command, "examples.SubmitPayment")
        .await;
    world.handle_result(result);
}

#[when(expr = "I handle a ConfirmPayment command with reference {string}")]
async fn handle_confirm_payment(world: &mut OrderAcceptanceWorld, payment_reference: String) {
    let command = ConfirmPayment { payment_reference };
    let result = world
        .execute_command(command, "examples.ConfirmPayment")
        .await;
    world.handle_result(result);
}

#[when(expr = "I handle a CancelOrder command with reason {string}")]
async fn handle_cancel_order(world: &mut OrderAcceptanceWorld, reason: String) {
    let command = CancelOrder { reason };
    let result = world.execute_command(command, "examples.CancelOrder").await;
    world.handle_result(result);
}

#[when("I rebuild the order state")]
async fn rebuild_order_state(world: &mut OrderAcceptanceWorld) {
    let order_id = world.order_root();
    let client = world.client().await;
    let _ = client
        .query
        .query("order", order_id)
        .range(0)
        .get_event_book()
        .await;
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("the result is an OrderCreated event")]
async fn result_is_order_created(world: &mut OrderAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(OrderAcceptanceWorld::extract_event_type(event).contains("OrderCreated"));
}

#[then("the result is a LoyaltyDiscountApplied event")]
async fn result_is_loyalty_discount_applied(world: &mut OrderAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(OrderAcceptanceWorld::extract_event_type(event).contains("LoyaltyDiscountApplied"));
}

#[then("the result is a PaymentSubmitted event")]
async fn result_is_payment_submitted(world: &mut OrderAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(OrderAcceptanceWorld::extract_event_type(event).contains("PaymentSubmitted"));
}

#[then("the result is an OrderCompleted event")]
async fn result_is_order_completed(world: &mut OrderAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(OrderAcceptanceWorld::extract_event_type(event).contains("OrderCompleted"));
}

#[then("the result is an OrderCancelled event")]
async fn result_is_order_cancelled(world: &mut OrderAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(OrderAcceptanceWorld::extract_event_type(event).contains("OrderCancelled"));
}

#[then(expr = "the command fails with status {string}")]
async fn command_fails_with_status(world: &mut OrderAcceptanceWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
async fn error_message_contains(world: &mut OrderAcceptanceWorld, substring: String) {
    assert!(world.last_error.is_some(), "Expected error but got success");
    let error_msg = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected '{}' in '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the order event has customer_id {string}")]
async fn event_has_customer_id(world: &mut OrderAcceptanceWorld, customer_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = OrderCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.customer_id, customer_id);
}

#[then(expr = "the order event has {int} items")]
async fn event_has_item_count(world: &mut OrderAcceptanceWorld, count: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = OrderCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.items.len() as i32, count);
}

#[then(expr = "the order event has subtotal_cents {int}")]
async fn event_has_subtotal_cents(world: &mut OrderAcceptanceWorld, subtotal_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = OrderCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.subtotal_cents, subtotal_cents);
}

#[then(expr = "the order event has points_used {int}")]
async fn event_has_points_used(world: &mut OrderAcceptanceWorld, points_used: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event =
        LoyaltyDiscountApplied::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.points_used, points_used);
}

#[then(expr = "the order event has discount_cents {int}")]
async fn event_has_discount_cents(world: &mut OrderAcceptanceWorld, discount_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event =
        LoyaltyDiscountApplied::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.discount_cents, discount_cents);
}

#[then(expr = "the order event has payment_method {string}")]
async fn event_has_payment_method(world: &mut OrderAcceptanceWorld, payment_method: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = PaymentSubmitted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.payment_method, payment_method);
}

#[then(expr = "the order event has amount_cents {int}")]
async fn event_has_amount_cents(world: &mut OrderAcceptanceWorld, amount_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = PaymentSubmitted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.amount_cents, amount_cents);
}

#[then(expr = "the order event has final_total_cents {int}")]
async fn event_has_final_total_cents(world: &mut OrderAcceptanceWorld, final_total_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = OrderCompleted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.final_total_cents, final_total_cents);
}

#[then(expr = "the order event has loyalty_points_earned {int}")]
async fn event_has_loyalty_points_earned(
    world: &mut OrderAcceptanceWorld,
    loyalty_points_earned: i32,
) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = OrderCompleted::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.loyalty_points_earned, loyalty_points_earned);
}

#[then(expr = "the order event has reason {string}")]
async fn event_has_reason(world: &mut OrderAcceptanceWorld, reason: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = OrderCancelled::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.reason, reason);
}

// State assertions
#[then(expr = "the order state has customer_id {string}")]
async fn state_has_customer_id(world: &mut OrderAcceptanceWorld, customer_id: String) {
    let order_id = world.order_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("order", order_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if OrderAcceptanceWorld::extract_event_type(event_any).contains("OrderCreated") {
                let event = OrderCreated::decode(event_any.value.as_slice()).expect("decode");
                assert_eq!(event.customer_id, customer_id);
                return;
            }
        }
    }
    panic!("No OrderCreated event found");
}

#[then(expr = "the order state has subtotal_cents {int}")]
async fn state_has_subtotal_cents(world: &mut OrderAcceptanceWorld, subtotal_cents: i32) {
    let order_id = world.order_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("order", order_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if OrderAcceptanceWorld::extract_event_type(event_any).contains("OrderCreated") {
                let event = OrderCreated::decode(event_any.value.as_slice()).expect("decode");
                assert_eq!(event.subtotal_cents, subtotal_cents);
                return;
            }
        }
    }
    panic!("No OrderCreated event found");
}

#[then(expr = "the order state has status {string}")]
async fn state_has_status(world: &mut OrderAcceptanceWorld, status: String) {
    let order_id = world.order_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("order", order_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut current_status = "pending".to_string();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = OrderAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("PaymentSubmitted") {
                current_status = "payment_submitted".to_string();
            } else if event_type.contains("OrderCompleted") {
                current_status = "completed".to_string();
            } else if event_type.contains("OrderCancelled") {
                current_status = "cancelled".to_string();
            }
        }
    }
    assert_eq!(current_status, status);
}

#[then(expr = "the order state has loyalty_points_used {int}")]
async fn state_has_loyalty_points_used(world: &mut OrderAcceptanceWorld, loyalty_points_used: i32) {
    let order_id = world.order_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("order", order_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut points = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if OrderAcceptanceWorld::extract_event_type(event_any)
                .contains("LoyaltyDiscountApplied")
            {
                let event =
                    LoyaltyDiscountApplied::decode(event_any.value.as_slice()).expect("decode");
                points = event.points_used;
            }
        }
    }
    assert_eq!(points, loyalty_points_used);
}

#[then(expr = "the order state has discount_cents {int}")]
async fn state_has_discount_cents(world: &mut OrderAcceptanceWorld, discount_cents: i32) {
    let order_id = world.order_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("order", order_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut discount = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if OrderAcceptanceWorld::extract_event_type(event_any)
                .contains("LoyaltyDiscountApplied")
            {
                let event =
                    LoyaltyDiscountApplied::decode(event_any.value.as_slice()).expect("decode");
                discount = event.discount_cents;
            }
        }
    }
    assert_eq!(discount, discount_cents);
}

#[tokio::main]
async fn main() {
    OrderAcceptanceWorld::cucumber()
        .run("tests/features/order.feature")
        .await;
}
