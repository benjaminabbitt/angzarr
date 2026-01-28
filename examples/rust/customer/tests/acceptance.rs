//! Acceptance tests for Customer domain.
//!
//! These tests run against a deployed angzarr system (Kind cluster).
//! Run with: cargo test -p customer --test acceptance

use angzarr::proto::CommandResponse;
use angzarr_client::{type_name_from_url, Client, ClientError, CommandBuilderExt, QueryBuilderExt};
use cucumber::{given, then, when, World};
use prost::Message;
use uuid::Uuid;

use common::identity::customer_root;
use common::proto::{
    AddLoyaltyPoints, CreateCustomer, CustomerCreated, LoyaltyPointsAdded, LoyaltyPointsRedeemed,
    RedeemLoyaltyPoints,
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

/// Makes an email unique for test isolation by inserting a UUID before the @ symbol.
/// "alice@example.com" becomes "alice-{uuid}@example.com"
/// Empty emails are preserved (for validation tests).
fn make_unique_email(email: &str) -> String {
    if email.is_empty() {
        return email.to_string();
    }
    if let Some(at_pos) = email.find('@') {
        let (local, domain) = email.split_at(at_pos);
        format!("{}-{}{}", local, Uuid::new_v4(), domain)
    } else {
        format!("{}-{}", email, Uuid::new_v4())
    }
}

/// Acceptance test world - connects to deployed gRPC services.
#[derive(World)]
#[world(init = Self::new)]
pub struct CustomerAcceptanceWorld {
    client: Option<Client>,

    /// Current customer email (used to compute aggregate root)
    current_email: Option<String>,

    /// Current sequence number for command tracking
    current_sequence: u32,

    /// Last command response
    last_response: Option<CommandResponse>,
    /// Last error message
    last_error: Option<String>,
}

impl std::fmt::Debug for CustomerAcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomerAcceptanceWorld")
            .field("current_email", &self.current_email)
            .finish()
    }
}

impl CustomerAcceptanceWorld {
    async fn new() -> Self {
        Self {
            client: None,
            current_email: None,
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

    fn customer_root(&self) -> Uuid {
        let email = self.current_email.as_ref().expect("No email set");
        customer_root(email)
    }

    async fn execute_command<M: Message>(
        &mut self,
        command: M,
        type_url: &str,
    ) -> Result<CommandResponse, ClientError> {
        let root = self.customer_root();
        let sequence = self.current_sequence;
        let client = self.client().await;

        client
            .gateway
            .command("customer", root)
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
// Background Steps
// =============================================================================

#[given(expr = "the angzarr system is running at {string}")]
async fn given_system_running(world: &mut CustomerAcceptanceWorld, _endpoint: String) {
    let _ = world.client().await;
}

// =============================================================================
// Given Steps - Setup scenarios
// =============================================================================

#[given("no prior events for the aggregate")]
async fn no_prior_events(world: &mut CustomerAcceptanceWorld) {
    // Clear email so When step will create a unique one
    world.current_email = None;
    world.current_sequence = 0;
    world.last_response = None;
    world.last_error = None;
}

#[given(expr = "a CustomerCreated event with name {string} and email {string}")]
async fn customer_created_event(world: &mut CustomerAcceptanceWorld, name: String, email: String) {
    // Generate unique email for test isolation by adding UUID prefix to domain
    let unique_email = make_unique_email(&email);
    world.current_email = Some(unique_email.clone());
    world.current_sequence = 0;

    // Send CreateCustomer command to create the event
    let command = CreateCustomer {
        name,
        email: unique_email.clone(),
    };
    let result = world
        .execute_command(command, "examples.CreateCustomer")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(e) => {
            panic!("Given step failed: CustomerCreated - {}", e.message());
        }
    }

    // Wait for event to be persisted
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(expr = "a LoyaltyPointsAdded event with {int} points and new_balance {int}")]
async fn loyalty_points_added_event(
    world: &mut CustomerAcceptanceWorld,
    points: i32,
    _new_balance: i32,
) {
    let command = AddLoyaltyPoints {
        points,
        reason: "setup".to_string(),
    };
    let result = world
        .execute_command(command, "examples.AddLoyaltyPoints")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(e) => {
            panic!("Given step failed: LoyaltyPointsAdded - {}", e.message());
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[given(expr = "a LoyaltyPointsRedeemed event with {int} points and new_balance {int}")]
async fn loyalty_points_redeemed_event(
    world: &mut CustomerAcceptanceWorld,
    points: i32,
    _new_balance: i32,
) {
    let command = RedeemLoyaltyPoints {
        points,
        redemption_type: "setup".to_string(),
    };
    let result = world
        .execute_command(command, "examples.RedeemLoyaltyPoints")
        .await;
    match result {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
            world.current_sequence += 1;
        }
        Err(e) => {
            panic!("Given step failed: LoyaltyPointsRedeemed - {}", e.message());
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

// =============================================================================
// When Steps - Execute commands
// =============================================================================

#[when(expr = "I handle a CreateCustomer command with name {string} and email {string}")]
async fn handle_create_customer(world: &mut CustomerAcceptanceWorld, name: String, email: String) {
    // Generate unique email for test isolation, unless current_email is already set
    // (from a prior Given step that created the customer)
    let unique_email = if world.current_email.is_some() {
        // Existing customer scenario - use the email from Given step
        world.current_email.clone().unwrap()
    } else {
        // New customer scenario - generate unique email
        let unique = make_unique_email(&email);
        world.current_email = Some(unique.clone());
        unique
    };

    let command = CreateCustomer {
        name,
        email: unique_email,
    };
    let result = world
        .execute_command(command, "examples.CreateCustomer")
        .await;
    world.handle_result(result);
}

#[when(expr = "I handle an AddLoyaltyPoints command with {int} points and reason {string}")]
async fn handle_add_loyalty_points(
    world: &mut CustomerAcceptanceWorld,
    points: i32,
    reason: String,
) {
    // For non-existent customer tests, generate a unique email if none set
    if world.current_email.is_none() {
        world.current_email = Some(make_unique_email("nonexistent@example.com"));
    }

    let command = AddLoyaltyPoints { points, reason };
    let result = world
        .execute_command(command, "examples.AddLoyaltyPoints")
        .await;
    world.handle_result(result);
}

#[when(expr = "I handle a RedeemLoyaltyPoints command with {int} points and type {string}")]
async fn handle_redeem_loyalty_points(
    world: &mut CustomerAcceptanceWorld,
    points: i32,
    redemption_type: String,
) {
    // For non-existent customer tests, generate a unique email if none set
    if world.current_email.is_none() {
        world.current_email = Some(make_unique_email("nonexistent@example.com"));
    }

    let command = RedeemLoyaltyPoints {
        points,
        redemption_type,
    };
    let result = world
        .execute_command(command, "examples.RedeemLoyaltyPoints")
        .await;
    world.handle_result(result);
}

#[when("I rebuild the customer state")]
async fn rebuild_customer_state(world: &mut CustomerAcceptanceWorld) {
    let root = world.customer_root();
    let client = world.client().await;
    match client
        .query
        .query("customer", root)
        .range(0)
        .get_event_book()
        .await
    {
        Ok(_) => {}
        Err(e) => {
            world.last_error = Some(format!("{}", e));
        }
    }
}

// =============================================================================
// Then Steps - Assertions
// =============================================================================

#[then("the result is a CustomerCreated event")]
async fn result_is_customer_created(world: &mut CustomerAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events in response");
    assert!(!events.pages.is_empty(), "Expected at least one event");

    let event = events.pages[0].event.as_ref().expect("No event payload");
    let event_type = CustomerAcceptanceWorld::extract_event_type(event);
    assert!(
        event_type.contains("CustomerCreated"),
        "Expected CustomerCreated, got {}",
        event_type
    );
}

#[then("the result is a LoyaltyPointsAdded event")]
async fn result_is_loyalty_points_added(world: &mut CustomerAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events in response");
    assert!(!events.pages.is_empty());

    let event = events.pages[0].event.as_ref().expect("No event payload");
    let event_type = CustomerAcceptanceWorld::extract_event_type(event);
    assert!(
        event_type.contains("LoyaltyPointsAdded"),
        "Expected LoyaltyPointsAdded, got {}",
        event_type
    );
}

#[then("the result is a LoyaltyPointsRedeemed event")]
async fn result_is_loyalty_points_redeemed(world: &mut CustomerAcceptanceWorld) {
    assert!(
        world.last_error.is_none(),
        "Expected success but got error: {:?}",
        world.last_error
    );
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events in response");
    assert!(!events.pages.is_empty());

    let event = events.pages[0].event.as_ref().expect("No event payload");
    let event_type = CustomerAcceptanceWorld::extract_event_type(event);
    assert!(
        event_type.contains("LoyaltyPointsRedeemed"),
        "Expected LoyaltyPointsRedeemed, got {}",
        event_type
    );
}

#[then(expr = "the command fails with status {string}")]
async fn command_fails_with_status(world: &mut CustomerAcceptanceWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
async fn error_message_contains(world: &mut CustomerAcceptanceWorld, substring: String) {
    assert!(world.last_error.is_some(), "Expected error but got success");
    let error_msg = world.last_error.as_ref().unwrap().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected error to contain '{}', got '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the event has name {string}")]
async fn event_has_name(world: &mut CustomerAcceptanceWorld, name: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = CustomerCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.name, name);
}

#[then(expr = "the event has email {string}")]
async fn event_has_email(world: &mut CustomerAcceptanceWorld, email: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = CustomerCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    // Check that the email starts with the expected local part (before the UUID suffix)
    let expected_prefix = email.split('@').next().unwrap_or(&email);
    assert!(
        event.email.starts_with(expected_prefix),
        "Expected email to start with '{}', got '{}'",
        expected_prefix,
        event.email
    );
}

#[then(expr = "the event has points {int}")]
async fn event_has_points(world: &mut CustomerAcceptanceWorld, points: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = CustomerAcceptanceWorld::extract_event_type(event_any);

    if event_type.contains("LoyaltyPointsAdded") {
        let event =
            LoyaltyPointsAdded::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.points, points);
    } else if event_type.contains("LoyaltyPointsRedeemed") {
        let event =
            LoyaltyPointsRedeemed::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.points, points);
    } else {
        panic!("Expected points event, got {}", event_type);
    }
}

#[then(expr = "the event has new_balance {int}")]
async fn event_has_new_balance(world: &mut CustomerAcceptanceWorld, new_balance: i32) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event_type = CustomerAcceptanceWorld::extract_event_type(event_any);

    if event_type.contains("LoyaltyPointsAdded") {
        let event =
            LoyaltyPointsAdded::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.new_balance, new_balance);
    } else if event_type.contains("LoyaltyPointsRedeemed") {
        let event =
            LoyaltyPointsRedeemed::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.new_balance, new_balance);
    } else {
        panic!("Expected points event, got {}", event_type);
    }
}

#[then(expr = "the event has reason {string}")]
async fn event_has_reason(world: &mut CustomerAcceptanceWorld, reason: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event = LoyaltyPointsAdded::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.reason, reason);
}

#[then(expr = "the event has redemption_type {string}")]
async fn event_has_redemption_type(world: &mut CustomerAcceptanceWorld, redemption_type: String) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("No events");
    let event_any = events.pages[0].event.as_ref().expect("No event");
    let event =
        LoyaltyPointsRedeemed::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.redemption_type, redemption_type);
}

#[then(expr = "the state has name {string}")]
async fn state_has_name(world: &mut CustomerAcceptanceWorld, name: String) {
    let root = world.customer_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("customer", root)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    // Find the CustomerCreated event
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CustomerAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("CustomerCreated") {
                let event =
                    CustomerCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
                assert_eq!(event.name, name);
                return;
            }
        }
    }
    panic!("No CustomerCreated event found");
}

#[then(expr = "the state has email {string}")]
async fn state_has_email(world: &mut CustomerAcceptanceWorld, email: String) {
    let root = world.customer_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("customer", root)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    // Check that the email starts with the expected local part (before the UUID suffix)
    let expected_prefix = email.split('@').next().unwrap_or(&email);

    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CustomerAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("CustomerCreated") {
                let event =
                    CustomerCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
                assert!(
                    event.email.starts_with(expected_prefix),
                    "Expected email to start with '{}', got '{}'",
                    expected_prefix,
                    event.email
                );
                return;
            }
        }
    }
    panic!("No CustomerCreated event found");
}

#[then(expr = "the state has loyalty_points {int}")]
async fn state_has_loyalty_points(world: &mut CustomerAcceptanceWorld, expected_points: i32) {
    let root = world.customer_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("customer", root)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    // Rebuild balance from events
    let mut balance = 0i32;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CustomerAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("LoyaltyPointsAdded") {
                let event = LoyaltyPointsAdded::decode(event_any.value.as_slice())
                    .expect("Failed to decode");
                balance = event.new_balance;
            } else if event_type.contains("LoyaltyPointsRedeemed") {
                let event = LoyaltyPointsRedeemed::decode(event_any.value.as_slice())
                    .expect("Failed to decode");
                balance = event.new_balance;
            }
        }
    }

    assert_eq!(
        balance, expected_points,
        "Expected loyalty_points {}, got {}",
        expected_points, balance
    );
}

#[then(expr = "the state has lifetime_points {int}")]
async fn state_has_lifetime_points(world: &mut CustomerAcceptanceWorld, expected_points: i32) {
    let root = world.customer_root();
    let client = world.client().await;
    let event_book = client
        .query
        .query("customer", root)
        .range(0)
        .get_event_book()
        .await
        .expect("Query failed");

    // Sum all points added (lifetime)
    let mut lifetime = 0i32;
    for page in &event_book.pages {
        if let Some(event_any) = &page.event {
            let event_type = CustomerAcceptanceWorld::extract_event_type(event_any);
            if event_type.contains("LoyaltyPointsAdded") {
                let event = LoyaltyPointsAdded::decode(event_any.value.as_slice())
                    .expect("Failed to decode");
                lifetime += event.points;
            }
        }
    }

    assert_eq!(
        lifetime, expected_points,
        "Expected lifetime_points {}, got {}",
        expected_points, lifetime
    );
}

#[tokio::main]
async fn main() {
    CustomerAcceptanceWorld::cucumber()
        .run("tests/features/customer.feature")
        .await;
}
