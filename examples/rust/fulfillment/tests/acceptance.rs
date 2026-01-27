//! Acceptance tests for Fulfillment domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p fulfillment --test acceptance

use cucumber::{given, then, when, World};
use prost::Message;
use tonic::transport::Channel;
use uuid::Uuid;

use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_query_client::EventQueryClient,
    CommandBook, CommandPage, CommandResponse, Cover, Query, Uuid as ProtoUuid,
};

use common::proto::{
    CreateShipment, Delivered, ItemsPacked, ItemsPicked, MarkPacked, MarkPicked, RecordDelivery,
    Ship, ShipmentCreated, Shipped,
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
pub struct FulfillmentAcceptanceWorld {
    gateway_endpoint: String,
    gateway_client: Option<CommandGatewayClient<Channel>>,
    query_client: Option<EventQueryClient<Channel>>,
    current_fulfillment_id: Option<Uuid>,
    current_sequence: u32,
    last_response: Option<CommandResponse>,
    last_error: Option<String>,
}

impl std::fmt::Debug for FulfillmentAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FulfillmentAcceptanceWorld")
            .field("gateway_endpoint", &self.gateway_endpoint)
            .field("current_fulfillment_id", &self.current_fulfillment_id)
            .finish()
    }
}

impl FulfillmentAcceptanceWorld {
    async fn new() -> Self {
        Self {
            gateway_endpoint: get_gateway_endpoint(),
            gateway_client: None,
            query_client: None,
            current_fulfillment_id: None,
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

    fn fulfillment_root(&self) -> Uuid {
        self.current_fulfillment_id.expect("No fulfillment ID set")
    }

    fn build_cover(&self) -> Cover {
        Cover {
            domain: "fulfillment".to_string(),
            root: Some(ProtoUuid {
                value: self.fulfillment_root().as_bytes().to_vec(),
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
    let command_book = world.build_command_book(command, "examples.CreateShipment");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(status) => {
            panic!("Given step failed: ShipmentCreated - {}", status.message());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given("an ItemsPicked event")]
async fn items_picked_event(world: &mut FulfillmentAcceptanceWorld) {
    let command = MarkPicked {
        picker_id: "PICKER-TEST".to_string(),
    };
    let command_book = world.build_command_book(command, "examples.MarkPicked");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.MarkPacked");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.Ship");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.RecordDelivery");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
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
    let command_book = world.build_command_book(command, "examples.CreateShipment");

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

#[when(expr = "I handle a MarkPicked command with picker_id {string}")]
async fn handle_mark_picked(world: &mut FulfillmentAcceptanceWorld, picker_id: String) {
    let command = MarkPicked { picker_id };
    let command_book = world.build_command_book(command, "examples.MarkPicked");

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

#[when(expr = "I handle a MarkPacked command with packer_id {string}")]
async fn handle_mark_packed(world: &mut FulfillmentAcceptanceWorld, packer_id: String) {
    let command = MarkPacked { packer_id };
    let command_book = world.build_command_book(command, "examples.MarkPacked");

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
    let command_book = world.build_command_book(command, "examples.Ship");

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

#[when(expr = "I handle a RecordDelivery command with signature {string}")]
async fn handle_record_delivery(world: &mut FulfillmentAcceptanceWorld, signature: String) {
    let command = RecordDelivery { signature };
    let command_book = world.build_command_book(command, "examples.RecordDelivery");

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

#[when("I rebuild the fulfillment state")]
async fn rebuild_fulfillment_state(world: &mut FulfillmentAcceptanceWorld) {
    let query = Query {
        domain: "fulfillment".to_string(),
        root: Some(ProtoUuid {
            value: world.fulfillment_root().as_bytes().to_vec(),
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
    let query = Query {
        domain: "fulfillment".to_string(),
        root: Some(ProtoUuid {
            value: world.fulfillment_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

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
    let query = Query {
        domain: "fulfillment".to_string(),
        root: Some(ProtoUuid {
            value: world.fulfillment_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

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
    let query = Query {
        domain: "fulfillment".to_string(),
        root: Some(ProtoUuid {
            value: world.fulfillment_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

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
