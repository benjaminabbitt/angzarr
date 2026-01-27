//! Acceptance tests for Inventory domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p inventory-svc --test acceptance

use cucumber::{given, then, when, World};
use prost::Message;
use tonic::transport::Channel;
use uuid::Uuid;

use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_query_client::EventQueryClient,
    CommandBook, CommandPage, CommandResponse, Cover, Query, Uuid as ProtoUuid,
};

use common::proto::{
    CommitReservation, InitializeStock, LowStockAlert, ReceiveStock, ReleaseReservation,
    ReservationCommitted, ReservationReleased, ReserveStock, StockInitialized, StockReceived,
    StockReserved,
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
pub struct InventoryAcceptanceWorld {
    gateway_endpoint: String,
    gateway_client: Option<CommandGatewayClient<Channel>>,
    query_client: Option<EventQueryClient<Channel>>,
    current_inventory_id: Option<Uuid>,
    last_response: Option<CommandResponse>,
    last_error: Option<String>,
    current_on_hand: i32,
    current_reserved: i32,
    /// Current sequence number for the aggregate (tracks next expected sequence)
    current_sequence: u32,
}

impl std::fmt::Debug for InventoryAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InventoryAcceptanceWorld")
            .field("gateway_endpoint", &self.gateway_endpoint)
            .field("current_inventory_id", &self.current_inventory_id)
            .finish()
    }
}

impl InventoryAcceptanceWorld {
    async fn new() -> Self {
        Self {
            gateway_endpoint: get_gateway_endpoint(),
            gateway_client: None,
            query_client: None,
            current_inventory_id: None,
            last_response: None,
            last_error: None,
            current_on_hand: 0,
            current_reserved: 0,
            current_sequence: 0,
        }
    }

    /// Update sequence after a successful command that emitted events
    fn update_sequence_from_response(&mut self) {
        if let Some(response) = &self.last_response {
            if let Some(events) = &response.events {
                self.current_sequence += events.pages.len() as u32;
            }
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

    fn inventory_root(&self) -> Uuid {
        self.current_inventory_id.expect("No inventory ID set")
    }

    fn build_cover(&self) -> Cover {
        Cover {
            domain: "inventory".to_string(),
            root: Some(ProtoUuid {
                value: self.inventory_root().as_bytes().to_vec(),
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

#[given("no prior events for the inventory aggregate")]
async fn no_prior_events(world: &mut InventoryAcceptanceWorld) {
    world.current_inventory_id = Some(Uuid::new_v4());
    world.last_response = None;
    world.last_error = None;
    world.current_on_hand = 0;
    world.current_reserved = 0;
    world.current_sequence = 0;
}

#[given(expr = "a StockInitialized event with product_id {string} and quantity {int}")]
async fn stock_initialized_event(
    world: &mut InventoryAcceptanceWorld,
    product_id: String,
    quantity: i32,
) {
    if world.current_inventory_id.is_none() {
        world.current_inventory_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }

    let command = InitializeStock {
        product_id,
        quantity,
        low_stock_threshold: 0,
    };
    let command_book = world.build_command_book(command, "examples.InitializeStock");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.current_on_hand = quantity;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(
    expr = "a StockInitialized event with product_id {string} and quantity {int} and low_stock_threshold {int}"
)]
async fn stock_initialized_event_with_threshold(
    world: &mut InventoryAcceptanceWorld,
    product_id: String,
    quantity: i32,
    low_stock_threshold: i32,
) {
    if world.current_inventory_id.is_none() {
        world.current_inventory_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }

    let command = InitializeStock {
        product_id,
        quantity,
        low_stock_threshold,
    };
    let command_book = world.build_command_book(command, "examples.InitializeStock");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.current_on_hand = quantity;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(expr = "a StockReceived event with quantity {int}")]
async fn stock_received_event(world: &mut InventoryAcceptanceWorld, quantity: i32) {
    let command = ReceiveStock {
        quantity,
        reference: "PO-TEST".to_string(),
    };
    let command_book = world.build_command_book(command, "examples.ReceiveStock");

    let client = world.get_gateway_client().await;
    if let Ok(response) = client.execute(command_book).await {
        world.last_response = Some(response.into_inner());
        world.update_sequence_from_response();
    }
    world.current_on_hand += quantity;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(expr = "a StockReserved event with quantity {int} and order_id {string}")]
async fn stock_reserved_event(
    world: &mut InventoryAcceptanceWorld,
    quantity: i32,
    order_id: String,
) {
    let command = ReserveStock {
        quantity,
        order_id: order_id.clone(),
    };
    let command_book = world.build_command_book(command, "examples.ReserveStock");

    let client = world.get_gateway_client().await;
    if let Ok(response) = client.execute(command_book).await {
        let resp = response.into_inner();
        if let Some(events) = &resp.events {
            eprintln!(
                "DEBUG Given StockReserved: {} events returned, order_id={}, seq={}, inventory_id={:?}",
                events.pages.len(),
                order_id,
                world.current_sequence,
                world.current_inventory_id
            );
        }
        world.last_response = Some(resp);
        world.update_sequence_from_response();
    }
    world.current_reserved += quantity;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(expr = "a ReservationCommitted event with order_id {string}")]
async fn reservation_committed_event(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let command = CommitReservation { order_id };
    let command_book = world.build_command_book(command, "examples.CommitReservation");

    let client = world.get_gateway_client().await;
    if let Ok(response) = client.execute(command_book).await {
        world.last_response = Some(response.into_inner());
        world.update_sequence_from_response();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(expr = "a ReservationReleased event with order_id {string}")]
async fn reservation_released_event(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let command = ReleaseReservation { order_id };
    let command_book = world.build_command_book(command, "examples.ReleaseReservation");

    let client = world.get_gateway_client().await;
    if let Ok(response) = client.execute(command_book).await {
        world.last_response = Some(response.into_inner());
        world.update_sequence_from_response();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

// =============================================================================
// When Steps
// =============================================================================

#[when(expr = "I handle an InitializeStock command with product_id {string} and quantity {int}")]
async fn handle_initialize_stock(
    world: &mut InventoryAcceptanceWorld,
    product_id: String,
    quantity: i32,
) {
    if world.current_inventory_id.is_none() {
        world.current_inventory_id = Some(Uuid::new_v4());
        world.current_sequence = 0;
    }

    let command = InitializeStock {
        product_id,
        quantity,
        low_stock_threshold: 0,
    };
    let command_book = world.build_command_book(command, "examples.InitializeStock");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a ReceiveStock command with quantity {int} and reference {string}")]
async fn handle_receive_stock(
    world: &mut InventoryAcceptanceWorld,
    quantity: i32,
    reference: String,
) {
    let command = ReceiveStock {
        quantity,
        reference,
    };
    let command_book = world.build_command_book(command, "examples.ReceiveStock");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a ReserveStock command with quantity {int} and order_id {string}")]
async fn handle_reserve_stock(
    world: &mut InventoryAcceptanceWorld,
    quantity: i32,
    order_id: String,
) {
    eprintln!(
        "DEBUG When ReserveStock: seq={}, inventory_id={:?}, order_id={}",
        world.current_sequence, world.current_inventory_id, order_id
    );
    let command = ReserveStock {
        quantity,
        order_id: order_id.clone(),
    };
    let command_book = world.build_command_book(command, "examples.ReserveStock");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a ReleaseReservation command with order_id {string}")]
async fn handle_release_reservation(world: &mut InventoryAcceptanceWorld, order_id: String) {
    eprintln!(
        "DEBUG When ReleaseReservation: seq={}, inventory_id={:?}, order_id={}",
        world.current_sequence, world.current_inventory_id, order_id
    );
    let command = ReleaseReservation {
        order_id: order_id.clone(),
    };
    let command_book = world.build_command_book(command, "examples.ReleaseReservation");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a CommitReservation command with order_id {string}")]
async fn handle_commit_reservation(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let command = CommitReservation { order_id };
    let command_book = world.build_command_book(command, "examples.CommitReservation");

    let client = world.get_gateway_client().await;
    match client.execute(command_book).await {
        Ok(response) => {
            world.last_response = Some(response.into_inner());
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(status) => {
            world.last_error = Some(status.message().to_string());
            world.last_response = None;
        }
    }
}

#[when("I rebuild the inventory state")]
async fn rebuild_inventory_state(world: &mut InventoryAcceptanceWorld) {
    let query = Query {
        domain: "inventory".to_string(),
        root: Some(ProtoUuid {
            value: world.inventory_root().as_bytes().to_vec(),
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

#[then("the result is a StockInitialized event")]
async fn result_is_stock_initialized(world: &mut InventoryAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(InventoryAcceptanceWorld::extract_event_type(event).contains("StockInitialized"));
}

#[then("the result is a StockReceived event")]
async fn result_is_stock_received(world: &mut InventoryAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(InventoryAcceptanceWorld::extract_event_type(event).contains("StockReceived"));
}

#[then("the result is a StockReserved event")]
async fn result_is_stock_reserved(world: &mut InventoryAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(InventoryAcceptanceWorld::extract_event_type(event).contains("StockReserved"));
}

#[then("the result is a ReservationReleased event")]
async fn result_is_reservation_released(world: &mut InventoryAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(InventoryAcceptanceWorld::extract_event_type(event).contains("ReservationReleased"));
}

#[then("the result is a ReservationCommitted event")]
async fn result_is_reservation_committed(world: &mut InventoryAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(!events.pages.is_empty());
    let event = events.pages[0].event.as_ref().expect("No event");
    assert!(InventoryAcceptanceWorld::extract_event_type(event).contains("ReservationCommitted"));
}

#[then(expr = "the command fails with status {string}")]
async fn command_fails_with_status(world: &mut InventoryAcceptanceWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
async fn error_message_contains(world: &mut InventoryAcceptanceWorld, substring: String) {
    assert!(world.last_error.is_some(), "Expected error but got success");
    let error_msg = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected '{}' in '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the inventory event has product_id {string}")]
async fn event_has_product_id(world: &mut InventoryAcceptanceWorld, product_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = StockInitialized::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.product_id, product_id);
}

#[then(expr = "the inventory event has quantity {int}")]
async fn event_has_quantity(world: &mut InventoryAcceptanceWorld, quantity: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);

    if event_type.contains("StockInitialized") {
        let event = StockInitialized::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.quantity, quantity);
    } else if event_type.contains("StockReceived") {
        let event = StockReceived::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.quantity, quantity);
    } else if event_type.contains("StockReserved") {
        let event = StockReserved::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.quantity, quantity);
    } else if event_type.contains("ReservationReleased") {
        let event = ReservationReleased::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.quantity, quantity);
    } else if event_type.contains("ReservationCommitted") {
        let event = ReservationCommitted::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.quantity, quantity);
    } else {
        panic!("No quantity field in event type {}", event_type);
    }
}

#[then(expr = "the inventory event has new_on_hand {int}")]
async fn event_has_new_on_hand(world: &mut InventoryAcceptanceWorld, new_on_hand: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);

    if event_type.contains("StockReceived") {
        let event = StockReceived::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.new_on_hand, new_on_hand);
    } else if event_type.contains("ReservationCommitted") {
        let event = ReservationCommitted::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.new_on_hand, new_on_hand);
    } else {
        panic!("No new_on_hand field in event type {}", event_type);
    }
}

#[then(expr = "the inventory event has order_id {string}")]
async fn event_has_order_id(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);

    if event_type.contains("StockReserved") {
        let event = StockReserved::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.order_id, order_id);
    } else if event_type.contains("ReservationReleased") {
        let event = ReservationReleased::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.order_id, order_id);
    } else if event_type.contains("ReservationCommitted") {
        let event = ReservationCommitted::decode(event_any.value.as_slice()).expect("decode");
        assert_eq!(event.order_id, order_id);
    } else {
        panic!("No order_id field in event type {}", event_type);
    }
}

#[then(expr = "the inventory event has new_available {int}")]
async fn event_has_new_available(world: &mut InventoryAcceptanceWorld, new_available: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");

    // Debug: print all events in the response
    eprintln!(
        "DEBUG: Response has {} events, inventory_id={:?}",
        events.pages.len(),
        world.current_inventory_id
    );
    for (i, page) in events.pages.iter().enumerate() {
        if let Some(evt) = &page.event {
            let evt_type = InventoryAcceptanceWorld::extract_event_type(evt);
            eprintln!("  Event[{}]: type={}, seq={:?}", i, evt_type, page.sequence);
        }
    }

    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);

    if event_type.contains("StockReserved") {
        let event = StockReserved::decode(event_any.value.as_slice()).expect("decode");
        eprintln!(
            "DEBUG StockReserved: quantity={}, new_available={}, order_id={}",
            event.quantity, event.new_available, event.order_id
        );
        assert_eq!(event.new_available, new_available);
    } else if event_type.contains("ReservationReleased") {
        let event = ReservationReleased::decode(event_any.value.as_slice()).expect("decode");
        eprintln!(
            "DEBUG ReservationReleased: quantity={}, new_available={}, order_id={}",
            event.quantity, event.new_available, event.order_id
        );
        assert_eq!(event.new_available, new_available);
    } else {
        panic!("No new_available field in event type {}", event_type);
    }
}

#[then("a LowStockAlert event is also emitted")]
async fn low_stock_alert_emitted(world: &mut InventoryAcceptanceWorld) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    assert!(
        events.pages.len() >= 2,
        "Expected at least 2 events (StockReserved + LowStockAlert)"
    );

    let mut found_alert = false;
    for page in &events.pages {
        if let Some(event_any) = &page.event {
            if InventoryAcceptanceWorld::extract_event_type(event_any).contains("LowStockAlert") {
                let _alert =
                    LowStockAlert::decode(event_any.value.as_slice()).expect("Failed to decode");
                found_alert = true;
                break;
            }
        }
    }
    assert!(found_alert, "No LowStockAlert event found");
}

// State assertions

#[then(expr = "the inventory state has product_id {string}")]
async fn state_has_product_id(world: &mut InventoryAcceptanceWorld, product_id: String) {
    let query = Query {
        domain: "inventory".to_string(),
        root: Some(ProtoUuid {
            value: world.inventory_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            if InventoryAcceptanceWorld::extract_event_type(event_any).contains("StockInitialized")
            {
                let event =
                    StockInitialized::decode(event_any.value.as_slice()).expect("decode failed");
                assert_eq!(event.product_id, product_id);
                return;
            }
        }
    }
    panic!("No StockInitialized event found");
}

#[then(expr = "the inventory state has on_hand {int}")]
async fn state_has_on_hand(world: &mut InventoryAcceptanceWorld, on_hand: i32) {
    let query = Query {
        domain: "inventory".to_string(),
        root: Some(ProtoUuid {
            value: world.inventory_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut current_on_hand = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("StockInitialized") {
                let event = StockInitialized::decode(event_any.value.as_slice()).expect("decode");
                current_on_hand = event.quantity;
            } else if event_type.contains("StockReceived") {
                let event = StockReceived::decode(event_any.value.as_slice()).expect("decode");
                current_on_hand = event.new_on_hand;
            } else if event_type.contains("ReservationCommitted") {
                let event =
                    ReservationCommitted::decode(event_any.value.as_slice()).expect("decode");
                current_on_hand = event.new_on_hand;
            }
        }
    }
    assert_eq!(current_on_hand, on_hand);
}

#[then(expr = "the inventory state has reserved {int}")]
async fn state_has_reserved(world: &mut InventoryAcceptanceWorld, reserved: i32) {
    let query = Query {
        domain: "inventory".to_string(),
        root: Some(ProtoUuid {
            value: world.inventory_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut current_reserved = 0;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("StockInitialized") {
                current_reserved = 0;
            } else if event_type.contains("StockReserved") {
                let event = StockReserved::decode(event_any.value.as_slice()).expect("decode");
                current_reserved += event.quantity;
            } else if event_type.contains("ReservationReleased") {
                let event =
                    ReservationReleased::decode(event_any.value.as_slice()).expect("decode");
                current_reserved -= event.quantity;
            } else if event_type.contains("ReservationCommitted") {
                let event =
                    ReservationCommitted::decode(event_any.value.as_slice()).expect("decode");
                current_reserved -= event.quantity;
            }
        }
    }
    assert_eq!(current_reserved, reserved);
}

#[then(expr = "the inventory state has available {int}")]
async fn state_has_available(world: &mut InventoryAcceptanceWorld, available: i32) {
    let query = Query {
        domain: "inventory".to_string(),
        root: Some(ProtoUuid {
            value: world.inventory_root().as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    };
    let client = world.get_query_client().await;
    let response = client.get_event_book(query).await.expect("Query failed");
    let event_book = response.into_inner();

    let mut current_on_hand = 0;
    let mut current_reserved = 0;

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = InventoryAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("StockInitialized") {
                let event = StockInitialized::decode(event_any.value.as_slice()).expect("decode");
                current_on_hand = event.quantity;
                current_reserved = 0;
            } else if event_type.contains("StockReceived") {
                let event = StockReceived::decode(event_any.value.as_slice()).expect("decode");
                current_on_hand = event.new_on_hand;
            } else if event_type.contains("StockReserved") {
                let event = StockReserved::decode(event_any.value.as_slice()).expect("decode");
                current_reserved += event.quantity;
            } else if event_type.contains("ReservationReleased") {
                let event =
                    ReservationReleased::decode(event_any.value.as_slice()).expect("decode");
                current_reserved -= event.quantity;
            } else if event_type.contains("ReservationCommitted") {
                let event =
                    ReservationCommitted::decode(event_any.value.as_slice()).expect("decode");
                current_on_hand = event.new_on_hand;
                current_reserved -= event.quantity;
            }
        }
    }
    let computed_available = current_on_hand - current_reserved;
    assert_eq!(computed_available, available);
}

#[tokio::main]
async fn main() {
    InventoryAcceptanceWorld::cucumber()
        .run("tests/features/inventory.feature")
        .await;
}
