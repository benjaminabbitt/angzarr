//! Acceptance tests for Inventory domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p inventory-svc --test acceptance

use angzarr::proto::CommandResponse;
use angzarr_client::{
    type_name_from_url, Client, ClientError, CommandBuilderExt, QueryBuilderExt,
};
use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

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
    client: Option<Client>,
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
            .field("current_inventory_id", &self.current_inventory_id)
            .finish()
    }
}

impl InventoryAcceptanceWorld {
    async fn new() -> Self {
        Self {
            client: None,
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

    fn inventory_root(&self) -> Uuid {
        self.current_inventory_id.expect("No inventory ID set")
    }

    async fn execute_command<M: Message>(
        &mut self,
        command: M,
        type_url: &str,
    ) -> Result<CommandResponse, ClientError> {
        let inventory_id = self.inventory_root();
        let sequence = self.current_sequence;
        let client = self.client().await;

        client
            .gateway
            .command("inventory", inventory_id)
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
    let result = world
        .execute_command(command, "examples.InitializeStock")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_on_hand = quantity;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
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
    let result = world
        .execute_command(command, "examples.InitializeStock")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_on_hand = quantity;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
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
    let result = world
        .execute_command(command, "examples.ReceiveStock")
        .await;
    if let Ok(response) = result {
        world.last_response = Some(response);
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
    let result = world
        .execute_command(command, "examples.ReserveStock")
        .await;
    if let Ok(response) = result {
        if let Some(events) = &response.events {
            eprintln!(
                "DEBUG Given StockReserved: {} events returned, order_id={}, seq={}, inventory_id={:?}",
                events.pages.len(),
                order_id,
                world.current_sequence,
                world.current_inventory_id
            );
        }
        world.last_response = Some(response);
        world.update_sequence_from_response();
    }
    world.current_reserved += quantity;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(expr = "a ReservationCommitted event with order_id {string}")]
async fn reservation_committed_event(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let command = CommitReservation { order_id };
    let result = world
        .execute_command(command, "examples.CommitReservation")
        .await;
    if let Ok(response) = result {
        world.last_response = Some(response);
        world.update_sequence_from_response();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[given(expr = "a ReservationReleased event with order_id {string}")]
async fn reservation_released_event(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let command = ReleaseReservation { order_id };
    let result = world
        .execute_command(command, "examples.ReleaseReservation")
        .await;
    if let Ok(response) = result {
        world.last_response = Some(response);
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
    let result = world
        .execute_command(command, "examples.InitializeStock")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
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
    let result = world
        .execute_command(command, "examples.ReceiveStock")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
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
    let result = world
        .execute_command(command, "examples.ReserveStock")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
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
    let result = world
        .execute_command(command, "examples.ReleaseReservation")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
            world.last_response = None;
        }
    }
}

#[when(expr = "I handle a CommitReservation command with order_id {string}")]
async fn handle_commit_reservation(world: &mut InventoryAcceptanceWorld, order_id: String) {
    let command = CommitReservation { order_id };
    let result = world
        .execute_command(command, "examples.CommitReservation")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.update_sequence_from_response();
        }
        Err(e) => {
            world.last_error = Some(e.message());
            world.last_response = None;
        }
    }
}

#[when("I rebuild the inventory state")]
async fn rebuild_inventory_state(world: &mut InventoryAcceptanceWorld) {
    let inventory_id = world.inventory_root();
    let client = world.client().await;
    let _ = client.query.query("inventory", inventory_id).range(0).get_event_book().await;
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
    let inventory_id = world.inventory_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("inventory", inventory_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

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
    let inventory_id = world.inventory_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("inventory", inventory_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

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
    let inventory_id = world.inventory_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("inventory", inventory_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

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
    let inventory_id = world.inventory_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("inventory", inventory_id)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

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
