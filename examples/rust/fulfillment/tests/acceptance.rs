//! Acceptance tests for Fulfillment domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p fulfillment --test acceptance

use angzarr::proto::CommandResponse;
use angzarr_client::{type_name_from_url, Client, ClientError, CommandBuilderExt, QueryBuilderExt};
use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use common::proto::{
    CreateShipment, Delivered, ItemsPacked, ItemsPicked, MarkPacked, MarkPicked, RecordDelivery,
    Ship, ShipmentCreated, Shipped,
};

/// Default Angzarr port - standard across all languages/containers
const DEFAULT_ANGZARR_PORT: u16 = 9084;

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
pub struct FulfillmentAcceptanceWorld {
    client: Option<Client>,
    current_fulfillment_id: Option<Uuid>,
    current_sequence: u32,
    last_response: Option<CommandResponse>,
    last_error: Option<String>,
}

impl std::fmt::Debug for FulfillmentAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FulfillmentAcceptanceWorld")
            .field("current_fulfillment_id", &self.current_fulfillment_id)
            .finish()
    }
}

impl FulfillmentAcceptanceWorld {
    async fn new() -> Self {
        Self {
            client: None,
            current_fulfillment_id: None,
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

    fn fulfillment_root(&self) -> Uuid {
        self.current_fulfillment_id.expect("No fulfillment ID set")
    }

    async fn execute_command<M: Message>(
        &mut self,
        command: M,
        type_url: &str,
    ) -> Result<CommandResponse, ClientError> {
        let fulfillment_id = self.fulfillment_root();
        let sequence = self.current_sequence;
        let client = self.client().await;

        client
            .gateway
            .command("fulfillment", fulfillment_id)
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

#[given("no prior events for the fulfillment aggregate")]
async fn no_prior_events(world: &mut FulfillmentAcceptanceWorld) {
    world.current_fulfillment_id = Some(Uuid::new_v4());
    world.current_sequence = 0;
    world.last_response = None;
    world.last_error = None;
}

#[given(expr = "a ShipmentCreated event with order_id {string}")]
async fn shipment_created_event(world: &mut FulfillmentAcceptanceWorld, order_id: String) {
    if world.current_fulfillment_id.is_none() {
        world.current_fulfillment_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }

    let command = CreateShipment { order_id };
    let result = world
        .execute_command(command, "examples.CreateShipment")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(e) => {
            panic!("Given step failed: ShipmentCreated - {}", e.message());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("an ItemsPicked event")]
async fn items_picked_event(world: &mut FulfillmentAcceptanceWorld) {
    let command = MarkPicked {
        picker_id: "PICKER-TEST".to_string(),
    };
    let result = world.execute_command(command, "examples.MarkPicked").await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: ItemsPicked - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("an ItemsPacked event")]
async fn items_packed_event(world: &mut FulfillmentAcceptanceWorld) {
    let command = MarkPacked {
        packer_id: "PACKER-TEST".to_string(),
    };
    let result = world.execute_command(command, "examples.MarkPacked").await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: ItemsPacked - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("a Shipped event")]
async fn shipped_event(world: &mut FulfillmentAcceptanceWorld) {
    let command = Ship {
        carrier: "TestCarrier".to_string(),
        tracking_number: "TRACK-TEST".to_string(),
    };
    let result = world.execute_command(command, "examples.Ship").await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: Shipped - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("a Delivered event")]
async fn delivered_event(world: &mut FulfillmentAcceptanceWorld) {
    let command = RecordDelivery {
        signature: "Test Signature".to_string(),
    };
    let result = world
        .execute_command(command, "examples.RecordDelivery")
        .await;
    match result {
        Ok(_) => world.current_sequence += 1,
        Err(e) => panic!("Given step failed: Delivered - {}", e.message()),
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

// =============================================================================
// When Steps
// =============================================================================

#[when(expr = "I handle a CreateShipment command with order_id {string}")]
async fn handle_create_shipment(world: &mut FulfillmentAcceptanceWorld, order_id: String) {
    if world.current_fulfillment_id.is_none() {
        world.current_fulfillment_id = Some(Uuid::new_v4());
    }

    let command = CreateShipment { order_id };
    let result = world
        .execute_command(command, "examples.CreateShipment")
        .await;
    world.handle_result(result);
}

#[when(expr = "I handle a MarkPicked command with picker_id {string}")]
async fn handle_mark_picked(world: &mut FulfillmentAcceptanceWorld, picker_id: String) {
    let command = MarkPicked { picker_id };
    let result = world.execute_command(command, "examples.MarkPicked").await;
    world.handle_result(result);
}

#[when(expr = "I handle a MarkPacked command with packer_id {string}")]
async fn handle_mark_packed(world: &mut FulfillmentAcceptanceWorld, packer_id: String) {
    let command = MarkPacked { packer_id };
    let result = world.execute_command(command, "examples.MarkPacked").await;
    world.handle_result(result);
}

#[when(expr = "I handle a Ship command with carrier {string} and tracking_number {string}")]
async fn handle_ship(
    world: &mut FulfillmentAcceptanceWorld,
    carrier: String,
    tracking_number: String,
) {
    let command = Ship {
        carrier,
        tracking_number,
    };
    let result = world.execute_command(command, "examples.Ship").await;
    world.handle_result(result);
}

#[when(expr = "I handle a RecordDelivery command with signature {string}")]
async fn handle_record_delivery(world: &mut FulfillmentAcceptanceWorld, signature: String) {
    let command = RecordDelivery { signature };
    let result = world
        .execute_command(command, "examples.RecordDelivery")
        .await;
    world.handle_result(result);
}

#[when("I rebuild the fulfillment state")]
async fn rebuild_fulfillment_state(world: &mut FulfillmentAcceptanceWorld) {
    let fulfillment_id = world.fulfillment_root();
    let client = world.client().await;
    let _ = client
        .query
        .query("fulfillment", fulfillment_id)
        .range(0)
        .get_event_book()
        .await;
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("the result is a ShipmentCreated event")]
async fn result_is_shipment_created(world: &mut FulfillmentAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(FulfillmentAcceptanceWorld::extract_event_type(event).contains("ShipmentCreated"));
}

#[then("the result is an ItemsPicked event")]
async fn result_is_items_picked(world: &mut FulfillmentAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(FulfillmentAcceptanceWorld::extract_event_type(event).contains("ItemsPicked"));
}

#[then("the result is an ItemsPacked event")]
async fn result_is_items_packed(world: &mut FulfillmentAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(FulfillmentAcceptanceWorld::extract_event_type(event).contains("ItemsPacked"));
}

#[then("the result is a Shipped event")]
async fn result_is_shipped(world: &mut FulfillmentAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(FulfillmentAcceptanceWorld::extract_event_type(event).contains("Shipped"));
}

#[then("the result is a Delivered event")]
async fn result_is_delivered(world: &mut FulfillmentAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(FulfillmentAcceptanceWorld::extract_event_type(event).contains("Delivered"));
}

#[then(expr = "the command fails with status {string}")]
async fn command_fails_with_status(world: &mut FulfillmentAcceptanceWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
async fn error_message_contains(world: &mut FulfillmentAcceptanceWorld, substring: String) {
    assert!(world.last_error.is_some(), "Expected error but got success");
    let error_msg = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected '{}' in '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the fulfillment event has order_id {string}")]
async fn event_has_order_id(world: &mut FulfillmentAcceptanceWorld, order_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ShipmentCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.order_id, order_id);
}

#[then(expr = "the fulfillment event has status {string}")]
async fn event_has_status(world: &mut FulfillmentAcceptanceWorld, status: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ShipmentCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.status, status);
}

#[then(expr = "the fulfillment event has picker_id {string}")]
async fn event_has_picker_id(world: &mut FulfillmentAcceptanceWorld, picker_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ItemsPicked::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.picker_id, picker_id);
}

#[then(expr = "the fulfillment event has packer_id {string}")]
async fn event_has_packer_id(world: &mut FulfillmentAcceptanceWorld, packer_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = ItemsPacked::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.packer_id, packer_id);
}

#[then(expr = "the fulfillment event has carrier {string}")]
async fn event_has_carrier(world: &mut FulfillmentAcceptanceWorld, carrier: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = Shipped::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.carrier, carrier);
}

#[then(expr = "the fulfillment event has tracking_number {string}")]
async fn event_has_tracking_number(
    world: &mut FulfillmentAcceptanceWorld,
    tracking_number: String,
) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = Shipped::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.tracking_number, tracking_number);
}

#[then(expr = "the fulfillment event has signature {string}")]
async fn event_has_signature(world: &mut FulfillmentAcceptanceWorld, signature: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = Delivered::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.signature, signature);
}

// State assertions

#[then(expr = "the fulfillment state has order_id {string}")]
async fn state_has_order_id(world: &mut FulfillmentAcceptanceWorld, order_id: String) {
    let fulfillment_id = world.fulfillment_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("fulfillment", fulfillment_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if FulfillmentAcceptanceWorld::extract_event_type(event_any).contains("ShipmentCreated")
            {
                let event =
                    ShipmentCreated::decode(event_any.value.as_slice()).expect("decode failed");
                assert_eq!(event.order_id, order_id);
                return;
            }
        }
    }
    panic!("No ShipmentCreated event found");
}

#[then(expr = "the fulfillment state has status {string}")]
async fn state_has_status(world: &mut FulfillmentAcceptanceWorld, status: String) {
    let fulfillment_id = world.fulfillment_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("fulfillment", fulfillment_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut current_status = String::new();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = FulfillmentAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("ShipmentCreated") {
                current_status = "pending".to_string();
            } else if event_type.contains("ItemsPicked") {
                current_status = "picking".to_string();
            } else if event_type.contains("ItemsPacked") {
                current_status = "packing".to_string();
            } else if event_type.contains("Shipped") {
                current_status = "shipped".to_string();
            } else if event_type.contains("Delivered") {
                current_status = "delivered".to_string();
            }
        }
    }
    assert_eq!(current_status, status);
}

#[then(expr = "the fulfillment state has tracking_number {string}")]
async fn state_has_tracking_number(
    world: &mut FulfillmentAcceptanceWorld,
    tracking_number: String,
) {
    let fulfillment_id = world.fulfillment_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("fulfillment", fulfillment_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    let mut current_tracking = String::new();
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if FulfillmentAcceptanceWorld::extract_event_type(event_any).contains("Shipped") {
                let event = Shipped::decode(event_any.value.as_slice()).expect("decode");
                current_tracking = event.tracking_number;
            }
        }
    }
    assert_eq!(current_tracking, tracking_number);
}

#[tokio::main]
async fn main() {
    FulfillmentAcceptanceWorld::cucumber()
        .run("tests/features/fulfillment.feature")
        .await;
}
