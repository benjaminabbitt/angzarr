//! Acceptance tests for Order domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p order --test acceptance

use cucumber::{gherkin::Step, given, then, when, World};
use prost::Message;
use tonic::transport::Channel;
use uuid::Uuid;

use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_query_client::EventQueryClient,
    CommandBook, CommandPage, CommandResponse, Cover, Query, Uuid as ProtoUuid,
};

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
    gateway_endpoint: String,
    gateway_client: Option<CommandGatewayClient<Channel>>,
    query_client: Option<EventQueryClient<Channel>>,
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
            .field("gateway_endpoint", &self.gateway_endpoint)
            .field("current_order_id", &self.current_order_id)
            .finish()
    }
}

impl OrderAcceptanceWorld {
    async fn new() -> Self {
        Self {
            gateway_endpoint: get_gateway_endpoint(),
            gateway_client: None,
            query_client: None,
            current_order_id: None,
            current_sequence: 0,
            current_subtotal: 0,
            current_discount: 0,
            last_response: None,
            last_error: None,
        }
    }

    async fn get_gateway_client(&mut self) -> &mut CommandGatewayClient<Channel> {
        if self.gateway_client.is_none() {
            let channel = Channel::from_shared(self.gateway_endpoint.clone())
                .expect("Invalid gateway endpoint")
                .connect()
                .await
                .expect("Failed to connect to gateway");
            self.gateway_client = Some(CommandGatewayClient::new(channel));
        }
        self.gateway_client.as_mut().unwrap()
    }

    async fn get_query_client(&mut self) -> &mut EventQueryClient<Channel> {
        if self.query_client.is_none() {
            let channel = Channel::from_shared(self.gateway_endpoint.clone())
                .expect("Invalid query endpoint")
                .connect()
                .await
                .expect("Failed to connect to query service");
            self.query_client = Some(EventQueryClient::new(channel));
        }
        self.query_client.as_mut().unwrap()
    }

    fn order_root(&self) -> Uuid {
        self.current_order_id.expect("No order ID set")
    }

    fn build_cover(&self) -> Cover {
        Cover {
            domain: "order".to_string(),
            root: Some(ProtoUuid {
                value: self.order_root().as_bytes().to_vec(),
            }),
        }
    }

    fn build_command_book(&self, command: impl Message, type_url: &str) -> CommandBook {
        let correlation_id = Uuid::new_v4().to_string();
        CommandBook {
            cover: Some(self.build_cover()),
            pages: vec![CommandPage {
                sequence: self.current_sequence,
                command: Some(prost_types::Any {
                    type_url: format!("type.googleapis.com/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
            correlation_id,
            saga_origin: None,
        }
    }

    fn extract_event_type(event: &prost_types::Any) -> String {
        event
            .type_url
            .rsplit('/')
            .next()
            .unwrap_or(&event.type_url)
            .to_string()
    }

    fn parse_items_from_table(step: &Step) -> Vec<LineItem> {
        let mut items = Vec::new();
        if let Some(table) = &step.table {
            // Skip header row
            for row in table.rows.iter().skip(1) {
                if row.len() >= 4 {
                    items.push(LineItem {
                        product_id: row[0].clone(),
                        name: row[1].clone(),
                        quantity: row[2].parse().unwrap_or(0),
                        unit_price_cents: row[3].parse().unwrap_or(0),
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

    // Create a simple order with one item that totals to the subtotal
    let command = CreateOrder {
        customer_id,
        items: vec![LineItem {
            product_id: "SETUP-ITEM".to_string(),
            name: "Setup Item".to_string(),
            quantity: 1,
            unit_price_cents: subtotal,
        }],
    };
    let command_book = world.build_command_book(command, "examples.CreateOrder");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(status) => {
            panic!("Given step failed: OrderCreated - {}", status.message());
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
    let command_book = world.build_command_book(command, "examples.ApplyLoyaltyDiscount");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.SubmitPayment");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.ConfirmPayment");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.CancelOrder");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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

    let command = CreateOrder { customer_id, items };
    let command_book = world.build_command_book(command, "examples.CreateOrder");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a CreateOrder command with customer_id {string} and no items")]
async fn handle_create_order_no_items(world: &mut OrderAcceptanceWorld, customer_id: String) {
    if world.current_order_id.is_none() {
        world.current_order_id = Some(Uuid::new_v4());
    }

    let command = CreateOrder {
        customer_id,
        items: vec![],
    };
    let command_book = world.build_command_book(command, "examples.CreateOrder");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
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
    let command_book = world.build_command_book(command, "examples.ApplyLoyaltyDiscount");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.current_discount = discount_cents;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
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
    let command_book = world.build_command_book(command, "examples.SubmitPayment");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a ConfirmPayment command with reference {string}")]
async fn handle_confirm_payment(world: &mut OrderAcceptanceWorld, payment_reference: String) {
    let command = ConfirmPayment { payment_reference };
    let command_book = world.build_command_book(command, "examples.ConfirmPayment");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a CancelOrder command with reason {string}")]
async fn handle_cancel_order(world: &mut OrderAcceptanceWorld, reason: String) {
    let command = CancelOrder { reason };
    let command_book = world.build_command_book(command, "examples.CancelOrder");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when("I rebuild the order state")]
async fn rebuild_order_state(world: &mut OrderAcceptanceWorld) {
    let query = Query {
        domain: "order".to_string(),
        root: Some(ProtoUuid {
            value: world.order_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };

    let client = world.get_query_client().await;
    let _ = client.get_event_book(query).await;
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
    let query = Query {
        domain: "order".to_string(),
        root: Some(ProtoUuid {
            value: world.order_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();
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
    let query = Query {
        domain: "order".to_string(),
        root: Some(ProtoUuid {
            value: world.order_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();
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
    let query = Query {
        domain: "order".to_string(),
        root: Some(ProtoUuid {
            value: world.order_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

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
    let query = Query {
        domain: "order".to_string(),
        root: Some(ProtoUuid {
            value: world.order_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

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
    let query = Query {
        domain: "order".to_string(),
        root: Some(ProtoUuid {
            value: world.order_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

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
