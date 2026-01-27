//! Acceptance tests for Cart domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p cart --test acceptance

use cucumber::{given, then, when, World};
use prost::Message;
use tonic::transport::Channel;
use uuid::Uuid;

use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_query_client::EventQueryClient,
    CommandBook, CommandPage, CommandResponse, Cover, Query, Uuid as ProtoUuid,
};

use common::proto::{
    AddItem, ApplyCoupon, CartCheckedOut, CartCleared, CartCreated, Checkout, ClearCart,
    CouponApplied, CreateCart, ItemAdded, ItemRemoved, QuantityUpdated, RemoveItem, UpdateQuantity,
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
pub struct CartAcceptanceWorld {
    gateway_endpoint: String,
    gateway_client: Option<CommandGatewayClient<Channel>>,
    query_client: Option<EventQueryClient<Channel>>,
    current_cart_id: Option<Uuid>,
    current_sequence: u32,
    last_response: Option<CommandResponse>,
    last_error: Option<String>,
}

impl std::fmt::Debug for CartAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CartAcceptanceWorld")
            .field("gateway_endpoint", &self.gateway_endpoint)
            .field("current_cart_id", &self.current_cart_id)
            .finish()
    }
}

impl CartAcceptanceWorld {
    async fn new() -> Self {
        Self {
            gateway_endpoint: get_gateway_endpoint(),
            gateway_client: None,
            query_client: None,
            current_cart_id: None,
            current_sequence: 0,
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

    fn cart_root(&self) -> Uuid {
        self.current_cart_id.expect("No cart ID set")
    }

    fn build_cover(&self) -> Cover {
        Cover {
            domain: "cart".to_string(),
            root: Some(ProtoUuid {
                value: self.cart_root().as_bytes().to_vec(),
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
}

// =============================================================================
// Given Steps
// =============================================================================

#[given("no prior events for the cart aggregate")]
async fn no_prior_events(world: &mut CartAcceptanceWorld) {
    world.current_cart_id = Some(Uuid::new_v4());
    world.current_sequence = 0;
    world.last_response = None;
    world.last_error = None;
}

#[given(expr = "a CartCreated event with customer_id {string}")]
async fn cart_created_event(world: &mut CartAcceptanceWorld, customer_id: String) {
    if world.current_cart_id.is_none() {
        world.current_cart_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }

    let command = CreateCart { customer_id };
    let command_book = world.build_command_book(command, "examples.CreateCart");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(
    expr = "an ItemAdded event with product_id {string} quantity {int} and unit_price_cents {int}"
)]
async fn item_added_event(
    world: &mut CartAcceptanceWorld,
    product_id: String,
    quantity: i32,
    unit_price_cents: i32,
) {
    let command = AddItem {
        product_id,
        name: "Test Item".to_string(),
        quantity,
        unit_price_cents,
    };
    let command_book = world.build_command_book(command, "examples.AddItem");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: ItemAdded - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(expr = "a CouponApplied event with code {string}")]
async fn coupon_applied_event(world: &mut CartAcceptanceWorld, code: String) {
    let command = ApplyCoupon {
        code,
        coupon_type: "percentage".to_string(),
        value: 10,
    };
    let command_book = world.build_command_book(command, "examples.ApplyCoupon");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: CouponApplied - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("a CartCheckedOut event")]
async fn cart_checked_out_event(world: &mut CartAcceptanceWorld) {
    let command = Checkout {};
    let command_book = world.build_command_book(command, "examples.Checkout");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: CartCheckedOut - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

// =============================================================================
// When Steps
// =============================================================================

#[when(expr = "I handle a CreateCart command with customer_id {string}")]
async fn handle_create_cart(world: &mut CartAcceptanceWorld, customer_id: String) {
    // Use existing cart ID if set (for duplicate tests), otherwise create new
    if world.current_cart_id.is_none() {
        world.current_cart_id = Some(Uuid::new_v4());
    }

    let command = CreateCart { customer_id };
    let command_book = world.build_command_book(command, "examples.CreateCart");

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

#[when(
    expr = "I handle an AddItem command with product_id {string} name {string} quantity {int} and unit_price_cents {int}"
)]
async fn handle_add_item(
    world: &mut CartAcceptanceWorld,
    product_id: String,
    name: String,
    quantity: i32,
    unit_price_cents: i32,
) {
    let command = AddItem {
        product_id,
        name,
        quantity,
        unit_price_cents,
    };
    let command_book = world.build_command_book(command, "examples.AddItem");

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

#[when(expr = "I handle an UpdateQuantity command with product_id {string} and new_quantity {int}")]
async fn handle_update_quantity(
    world: &mut CartAcceptanceWorld,
    product_id: String,
    new_quantity: i32,
) {
    let command = UpdateQuantity {
        product_id,
        new_quantity,
    };
    let command_book = world.build_command_book(command, "examples.UpdateQuantity");

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

#[when(expr = "I handle a RemoveItem command with product_id {string}")]
async fn handle_remove_item(world: &mut CartAcceptanceWorld, product_id: String) {
    let command = RemoveItem { product_id };
    let command_book = world.build_command_book(command, "examples.RemoveItem");

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

#[when(expr = "I handle an ApplyCoupon command with code {string} type {string} and value {int}")]
async fn handle_apply_coupon(
    world: &mut CartAcceptanceWorld,
    code: String,
    coupon_type: String,
    value: i32,
) {
    let command = ApplyCoupon {
        code,
        coupon_type,
        value,
    };
    let command_book = world.build_command_book(command, "examples.ApplyCoupon");

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

#[when("I handle a ClearCart command")]
async fn handle_clear_cart(world: &mut CartAcceptanceWorld) {
    let command = ClearCart {};
    let command_book = world.build_command_book(command, "examples.ClearCart");

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

#[when("I handle a Checkout command")]
async fn handle_checkout(world: &mut CartAcceptanceWorld) {
    let command = Checkout {};
    let command_book = world.build_command_book(command, "examples.Checkout");

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

#[when("I rebuild the cart state")]
async fn rebuild_cart_state(world: &mut CartAcceptanceWorld) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
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

#[then("the result is a CartCreated event")]
async fn result_is_cart_created(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("CartCreated"));
}

#[then("the result is an ItemAdded event")]
async fn result_is_item_added(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("ItemAdded"));
}

#[then("the result is a QuantityUpdated event")]
async fn result_is_quantity_updated(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("QuantityUpdated"));
}

#[then("the result is an ItemRemoved event")]
async fn result_is_item_removed(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("ItemRemoved"));
}

#[then("the result is a CouponApplied event")]
async fn result_is_coupon_applied(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("CouponApplied"));
}

#[then("the result is a CartCleared event")]
async fn result_is_cart_cleared(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("CartCleared"));
}

#[then("the result is a CartCheckedOut event")]
async fn result_is_cart_checked_out(world: &mut CartAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(CartAcceptanceWorld::extract_event_type(event).contains("CartCheckedOut"));
}

#[then(expr = "the command fails with status {string}")]
async fn command_fails_with_status(world: &mut CartAcceptanceWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
async fn error_message_contains(world: &mut CartAcceptanceWorld, substring: String) {
    assert!(world.last_error.is_some(), "Expected error but got success");
    let error_msg = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected '{}' in '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the cart event has customer_id {string}")]
async fn event_has_customer_id(world: &mut CartAcceptanceWorld, customer_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = CartCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.customer_id, customer_id);
}

#[then(expr = "the cart event has product_id {string}")]
async fn event_has_product_id(world: &mut CartAcceptanceWorld, product_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = CartAcceptanceWorld::extract_event_type(event_any);
    if event_type.contains("ItemAdded") {
        let event = ItemAdded::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.product_id, product_id);
    } else if event_type.contains("ItemRemoved") {
        let event = ItemRemoved::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.product_id, product_id);
    } else {
        panic!("Expected ItemAdded or ItemRemoved, got {}", event_type);
    }
}

#[then(expr = "the cart event has quantity {int}")]
async fn event_has_quantity(world: &mut CartAcceptanceWorld, quantity: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ItemAdded::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.quantity, quantity);
}

#[then(expr = "the cart event has new_quantity {int}")]
async fn event_has_new_quantity(world: &mut CartAcceptanceWorld, new_quantity: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = QuantityUpdated::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.new_quantity, new_quantity);
}

#[then(expr = "the cart event has new_subtotal {int}")]
async fn event_has_new_subtotal(world: &mut CartAcceptanceWorld, new_subtotal: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = CartAcceptanceWorld::extract_event_type(event_any);
    if event_type.contains("ItemAdded") {
        let event = ItemAdded::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.new_subtotal, new_subtotal);
    } else if event_type.contains("ItemRemoved") {
        let event = ItemRemoved::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.new_subtotal, new_subtotal);
    } else if event_type.contains("QuantityUpdated") {
        let event = QuantityUpdated::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.new_subtotal, new_subtotal);
    } else if event_type.contains("CartCleared") {
        let event = CartCleared::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.new_subtotal, new_subtotal);
    } else {
        panic!("No new_subtotal in event type {}", event_type);
    }
}

#[then(expr = "the cart event has coupon_code {string}")]
async fn event_has_coupon_code(world: &mut CartAcceptanceWorld, coupon_code: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = CouponApplied::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.coupon_code, coupon_code);
}

#[then(expr = "the cart event has discount_cents {int}")]
async fn event_has_discount_cents(world: &mut CartAcceptanceWorld, discount_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = CouponApplied::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.discount_cents, discount_cents);
}

#[then(expr = "the cart event has final_subtotal {int}")]
async fn event_has_final_subtotal(world: &mut CartAcceptanceWorld, final_subtotal: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = CartCheckedOut::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.final_subtotal, final_subtotal);
}

// State assertions
#[then(expr = "the cart state has customer_id {string}")]
async fn state_has_customer_id(world: &mut CartAcceptanceWorld, customer_id: String) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if CartAcceptanceWorld::extract_event_type(event_any).contains("CartCreated") {
                let event = CartCreated::decode(event_any.value.as_slice()).expect("decode");
                assert_eq!(event.customer_id, customer_id);
                return;
            }
        }
    }
    panic!("No CartCreated event found");
}

#[then(expr = "the cart state has {int} items")]
async fn state_has_item_count(world: &mut CartAcceptanceWorld, count: i32) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    // Rebuild item count from events
    let mut items: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CartAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("ItemAdded") {
                let event = ItemAdded::decode(event_any.value.as_slice()).expect("decode");
                items.insert(event.product_id, true);
            } else if event_type.contains("ItemRemoved") {
                let event = ItemRemoved::decode(event_any.value.as_slice()).expect("decode");
                items.remove(&event.product_id);
            } else if event_type.contains("CartCleared") {
                items.clear();
            }
        }
    }
    assert_eq!(items.len() as i32, count);
}

#[then(expr = "the cart state has subtotal {int}")]
async fn state_has_subtotal(world: &mut CartAcceptanceWorld, subtotal: i32) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut latest_subtotal = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CartAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("ItemAdded") {
                let event = ItemAdded::decode(event_any.value.as_slice()).expect("decode");
                latest_subtotal = event.new_subtotal;
            } else if event_type.contains("ItemRemoved") {
                let event = ItemRemoved::decode(event_any.value.as_slice()).expect("decode");
                latest_subtotal = event.new_subtotal;
            } else if event_type.contains("QuantityUpdated") {
                let event = QuantityUpdated::decode(event_any.value.as_slice()).expect("decode");
                latest_subtotal = event.new_subtotal;
            } else if event_type.contains("CartCleared") {
                let event = CartCleared::decode(event_any.value.as_slice()).expect("decode");
                latest_subtotal = event.new_subtotal;
            }
        }
    }
    assert_eq!(latest_subtotal, subtotal);
}

#[then(expr = "the cart state has status {string}")]
async fn state_has_status(world: &mut CartAcceptanceWorld, status: String) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut is_checked_out = false;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if CartAcceptanceWorld::extract_event_type(event_any).contains("CartCheckedOut") {
                is_checked_out = true;
            }
        }
    }
    let actual_status = if is_checked_out {
        "checked_out"
    } else {
        "active"
    };
    assert_eq!(actual_status, status);
}

#[then(expr = "the cart state has coupon_code {string}")]
async fn state_has_coupon_code(world: &mut CartAcceptanceWorld, coupon_code: String) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut latest_coupon = String::new();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CartAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("CouponApplied") {
                let event = CouponApplied::decode(event_any.value.as_slice()).expect("decode");
                latest_coupon = event.coupon_code;
            } else if event_type.contains("CartCleared") {
                latest_coupon = String::new();
            }
        }
    }
    assert_eq!(latest_coupon, coupon_code);
}

#[then(expr = "the cart state has discount_cents {int}")]
async fn state_has_discount_cents(world: &mut CartAcceptanceWorld, discount_cents: i32) {
    let query = Query {
        domain: "cart".to_string(),
        root: Some(ProtoUuid {
            value: world.cart_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut latest_discount = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CartAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("CouponApplied") {
                let event = CouponApplied::decode(event_any.value.as_slice()).expect("decode");
                latest_discount = event.discount_cents;
            } else if event_type.contains("CartCleared") {
                latest_discount = 0;
            }
        }
    }
    assert_eq!(latest_discount, discount_cents);
}

#[tokio::main]
async fn main() {
    CartAcceptanceWorld::cucumber()
        .run("tests/features/cart.feature")
        .await;
}
