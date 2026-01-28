//! Acceptance tests for Product domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p product --test acceptance

use angzarr::proto::CommandResponse;
use angzarr_client::{
    type_name_from_url, Client, ClientError, CommandBuilderExt, QueryBuilderExt,
};
use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use common::proto::{
    CreateProduct, Discontinue, PriceSet, ProductCreated, ProductDiscontinued, ProductUpdated,
    SetPrice, UpdateProduct,
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
pub struct ProductAcceptanceWorld {
    client: Option<Client>,
    current_product_id: Option<Uuid>,
    current_sequence: u32,
    last_response: Option<CommandResponse>,
    last_error: Option<String>,
}

impl std::fmt::Debug for ProductAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductAcceptanceWorld")
            .field("current_product_id", &self.current_product_id)
            .finish()
    }
}

impl ProductAcceptanceWorld {
    async fn new() -> Self {
        Self {
            client: None,
            current_product_id: None,
            current_sequence: 0,
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

    fn product_root(&self) -> Uuid {
        self.current_product_id.expect("No product ID set")
    }

    async fn execute_command<M: Message>(
        &mut self,
        command: M,
        type_url: &str,
    ) -> Result<CommandResponse, ClientError> {
        let product_id = self.product_root();
        let sequence = self.current_sequence;
        let client = self.client().await;

        client
            .gateway
            .command("product", product_id)
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
}

// =============================================================================
// Given Steps
// =============================================================================

#[given("no prior events for the product aggregate")]
async fn no_prior_events(world: &mut ProductAcceptanceWorld) {
    world.current_product_id = Some(Uuid::new_v4());
    world.current_sequence = 0;
    world.last_response = None;
    world.last_error = None;
}

#[given(expr = "a ProductCreated event with sku {string} name {string} and price_cents {int}")]
async fn product_created_event(
    world: &mut ProductAcceptanceWorld,
    sku: String,
    name: String,
    price_cents: i32,
) {
    if world.current_product_id.is_none() {
        world.current_product_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }

    let command = CreateProduct {
        sku,
        name,
        description: String::new(),
        price_cents,
    };
    let result = world.execute_command(command, "examples.CreateProduct").await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(e) => {
            panic!("Given step failed: ProductCreated - {}", e.message());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("a ProductDiscontinued event")]
async fn product_discontinued_event(world: &mut ProductAcceptanceWorld) {
    let command = Discontinue {
        reason: "setup".to_string(),
    };
    let result = world
        .execute_command(command, "examples.Discontinue")
        .await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: ProductDiscontinued - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(expr = "a PriceSet event with price_cents {int}")]
async fn price_set_event(world: &mut ProductAcceptanceWorld, price_cents: i32) {
    let command = SetPrice { price_cents };
    let result = world.execute_command(command, "examples.SetPrice").await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: PriceSet - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(expr = "a ProductUpdated event with name {string} and description {string}")]
async fn product_updated_event(
    world: &mut ProductAcceptanceWorld,
    name: String,
    description: String,
) {
    let command = UpdateProduct { name, description };
    let result = world
        .execute_command(command, "examples.UpdateProduct")
        .await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: ProductUpdated - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

// =============================================================================
// When Steps
// =============================================================================

#[when(
    expr = "I handle a CreateProduct command with sku {string} name {string} description {string} and price_cents {int}"
)]
async fn handle_create_product(
    world: &mut ProductAcceptanceWorld,
    sku: String,
    name: String,
    description: String,
    price_cents: i32,
) {
    if world.current_product_id.is_none() {
        world.current_product_id = Some(Uuid::new_v4());
    }

    let command = CreateProduct {
        sku,
        name,
        description,
        price_cents,
    };
    let result = world.execute_command(command, "examples.CreateProduct").await;
    world.handle_result(result);
}

#[when(expr = "I handle an UpdateProduct command with name {string} and description {string}")]
async fn handle_update_product(
    world: &mut ProductAcceptanceWorld,
    name: String,
    description: String,
) {
    let command = UpdateProduct { name, description };
    let result = world
        .execute_command(command, "examples.UpdateProduct")
        .await;
    world.handle_result(result);
}

#[when(expr = "I handle a SetPrice command with price_cents {int}")]
async fn handle_set_price(world: &mut ProductAcceptanceWorld, price_cents: i32) {
    let command = SetPrice { price_cents };
    let result = world.execute_command(command, "examples.SetPrice").await;
    world.handle_result(result);
}

#[when(expr = "I handle a Discontinue command with reason {string}")]
async fn handle_discontinue(world: &mut ProductAcceptanceWorld, reason: String) {
    let command = Discontinue { reason };
    let result = world
        .execute_command(command, "examples.Discontinue")
        .await;
    world.handle_result(result);
}

#[when("I rebuild the product state")]
async fn rebuild_product_state(world: &mut ProductAcceptanceWorld) {
    let product_id = world.product_root();
    let client = world.client().await;
    let _ = client.query.query("product", product_id).range(0).get_event_book().await;
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("the result is a ProductCreated event")]
async fn result_is_product_created(world: &mut ProductAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(ProductAcceptanceWorld::extract_event_type(event).contains("ProductCreated"));
}

#[then("the result is a ProductUpdated event")]
async fn result_is_product_updated(world: &mut ProductAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(ProductAcceptanceWorld::extract_event_type(event).contains("ProductUpdated"));
}

#[then("the result is a PriceSet event")]
async fn result_is_price_set(world: &mut ProductAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(ProductAcceptanceWorld::extract_event_type(event).contains("PriceSet"));
}

#[then("the result is a ProductDiscontinued event")]
async fn result_is_product_discontinued(world: &mut ProductAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(ProductAcceptanceWorld::extract_event_type(event).contains("ProductDiscontinued"));
}

#[then(expr = "the command fails with status {string}")]
async fn command_fails_with_status(world: &mut ProductAcceptanceWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
async fn error_message_contains(world: &mut ProductAcceptanceWorld, substring: String) {
    assert!(world.last_error.is_some(), "Expected error but got success");
    let error_msg = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected '{}' in '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the product event has sku {string}")]
async fn event_has_sku(world: &mut ProductAcceptanceWorld, sku: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ProductCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.sku, sku);
}

#[then(expr = "the product event has name {string}")]
async fn event_has_name(world: &mut ProductAcceptanceWorld, name: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = ProductAcceptanceWorld::extract_event_type(event_any);
    if event_type.contains("ProductCreated") {
        let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.name, name);
    } else {
        let event = ProductUpdated::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.name, name);
    }
}

#[then(expr = "the product event has description {string}")]
async fn event_has_description(world: &mut ProductAcceptanceWorld, description: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = ProductAcceptanceWorld::extract_event_type(event_any);
    if event_type.contains("ProductCreated") {
        let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.description, description);
    } else {
        let event = ProductUpdated::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.description, description);
    }
}

#[then(expr = "the product event has price_cents {int}")]
async fn event_has_price_cents(world: &mut ProductAcceptanceWorld, price_cents: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = ProductAcceptanceWorld::extract_event_type(event_any);
    if event_type.contains("ProductCreated") {
        let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.price_cents, price_cents);
    } else {
        let event = PriceSet::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.price_cents, price_cents);
    }
}

#[then(expr = "the product event has previous_price_cents {int}")]
async fn event_has_previous_price_cents(
    world: &mut ProductAcceptanceWorld,
    previous_price_cents: i32,
) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = PriceSet::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.previous_price_cents, previous_price_cents);
}

#[then(expr = "the product event has reason {string}")]
async fn event_has_reason(world: &mut ProductAcceptanceWorld, reason: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ProductDiscontinued::decode(event_any.value.as_slice()).expect("decode");
    assert_eq!(event.reason, reason);
}

#[then(expr = "the product state has sku {string}")]
async fn state_has_sku(world: &mut ProductAcceptanceWorld, sku: String) {
    let product_id = world.product_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("product", product_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if ProductAcceptanceWorld::extract_event_type(event_any).contains("ProductCreated") {
                let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
                assert_eq!(event.sku, sku);
                return;
            }
        }
    }
    panic!("No ProductCreated event found");
}

#[then(expr = "the product state has name {string}")]
async fn state_has_name(world: &mut ProductAcceptanceWorld, name: String) {
    let product_id = world.product_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("product", product_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut latest_name = String::new();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = ProductAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("ProductCreated") {
                let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
                latest_name = event.name;
            } else if event_type.contains("ProductUpdated") {
                let event = ProductUpdated::decode(event_any.value.as_slice()).expect("decode");
                latest_name = event.name;
            }
        }
    }
    assert_eq!(latest_name, name);
}

#[then(expr = "the product state has description {string}")]
async fn state_has_description(world: &mut ProductAcceptanceWorld, description: String) {
    let product_id = world.product_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("product", product_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut latest_desc = String::new();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = ProductAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("ProductCreated") {
                let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
                latest_desc = event.description;
            } else if event_type.contains("ProductUpdated") {
                let event = ProductUpdated::decode(event_any.value.as_slice()).expect("decode");
                latest_desc = event.description;
            }
        }
    }
    assert_eq!(latest_desc, description);
}

#[then(expr = "the product state has price_cents {int}")]
async fn state_has_price_cents(world: &mut ProductAcceptanceWorld, price_cents: i32) {
    let product_id = world.product_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("product", product_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut latest_price = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = ProductAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("ProductCreated") {
                let event = ProductCreated::decode(event_any.value.as_slice()).expect("decode");
                latest_price = event.price_cents;
            } else if event_type.contains("PriceSet") {
                let event = PriceSet::decode(event_any.value.as_slice()).expect("decode");
                latest_price = event.price_cents;
            }
        }
    }
    assert_eq!(latest_price, price_cents);
}

#[then(expr = "the product state has status {string}")]
async fn state_has_status(world: &mut ProductAcceptanceWorld, status: String) {
    let product_id = world.product_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("product", product_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut is_discontinued = false;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if ProductAcceptanceWorld::extract_event_type(event_any).contains("ProductDiscontinued")
            {
                is_discontinued = true;
            }
        }
    }
    let actual_status = if is_discontinued {
        "discontinued"
    } else {
        "active"
    };
    assert_eq!(actual_status, status);
}

#[tokio::main]
async fn main() {
    ProductAcceptanceWorld::cucumber()
        .run("tests/features/product.feature")
        .await;
}
