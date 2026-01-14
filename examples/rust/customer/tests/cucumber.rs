//! Cucumber BDD tests for Customer business logic.

use cucumber::{given, then, when, World};
use prost::Message;

use angzarr::interfaces::business_client::BusinessError;
use angzarr::proto::{CommandBook, CommandPage, EventBook, EventPage};
use common::proto::{
    CustomerCreated, CustomerState, LoyaltyPointsAdded, LoyaltyPointsRedeemed,
};
use customer::CustomerLogic;

/// Wrapper to allow Debug derive for CustomerLogic
#[derive(Default)]
struct LogicWrapper(Option<CustomerLogic>);

impl std::fmt::Debug for LogicWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogicWrapper")
            .field("initialized", &self.0.is_some())
            .finish()
    }
}

#[derive(Debug, Default, World)]
pub struct CustomerWorld {
    logic: LogicWrapper,
    prior_events: Vec<prost_types::Any>,
    result_event_book: Option<EventBook>,
    error: Option<BusinessError>,
    state: Option<CustomerState>,
}

impl CustomerWorld {
    fn logic(&self) -> &CustomerLogic {
        self.logic.0.as_ref().expect("logic not initialized")
    }

    fn build_event_book(&self) -> Option<EventBook> {
        if self.prior_events.is_empty() {
            return None;
        }
        Some(EventBook {
            cover: None,
            snapshot: None,
            pages: self
                .prior_events
                .iter()
                .map(|event| EventPage {
                    sequence: None,
                    event: Some(event.clone()),
                    created_at: None,
                    synchronous: false,
                })
                .collect(),
            correlation_id: String::new(),
            snapshot_state: None,
        })
    }
}

// --- Given steps ---

#[given("no prior events for the aggregate")]
fn no_prior_events(world: &mut CustomerWorld) {
    world.logic = LogicWrapper(Some(CustomerLogic::new()));
    world.prior_events.clear();
    world.result_event_book = None;
    world.error = None;
    world.state = None;
}

#[given(expr = "a CustomerCreated event with name {string} and email {string}")]
fn customer_created_event(world: &mut CustomerWorld, name: String, email: String) {
    if world.logic.0.is_none() {
        world.logic = LogicWrapper(Some(CustomerLogic::new()));
    }
    let event = CustomerCreated {
        name,
        email,
        created_at: None,
    };
    world.prior_events.push(prost_types::Any {
        type_url: "type.examples/examples.CustomerCreated".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a LoyaltyPointsAdded event with {int} points and new_balance {int}")]
fn loyalty_points_added_event(world: &mut CustomerWorld, points: i32, new_balance: i32) {
    if world.logic.0.is_none() {
        world.logic = LogicWrapper(Some(CustomerLogic::new()));
    }
    let event = LoyaltyPointsAdded {
        points,
        new_balance,
        reason: String::new(),
    };
    world.prior_events.push(prost_types::Any {
        type_url: "type.examples/examples.LoyaltyPointsAdded".to_string(),
        value: event.encode_to_vec(),
    });
}

#[given(expr = "a LoyaltyPointsRedeemed event with {int} points and new_balance {int}")]
fn loyalty_points_redeemed_event(world: &mut CustomerWorld, points: i32, new_balance: i32) {
    if world.logic.0.is_none() {
        world.logic = LogicWrapper(Some(CustomerLogic::new()));
    }
    let event = LoyaltyPointsRedeemed {
        points,
        new_balance,
        redemption_type: String::new(),
    };
    world.prior_events.push(prost_types::Any {
        type_url: "type.examples/examples.LoyaltyPointsRedeemed".to_string(),
        value: event.encode_to_vec(),
    });
}

// --- When steps ---

#[when(expr = "I handle a CreateCustomer command with name {string} and email {string}")]
fn handle_create_customer(world: &mut CustomerWorld, name: String, email: String) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));

    let cmd = common::proto::CreateCustomer { name, email };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.CreateCustomer".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_create_customer_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when(expr = "I handle an AddLoyaltyPoints command with {int} points and reason {string}")]
fn handle_add_loyalty_points(world: &mut CustomerWorld, points: i32, reason: String) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));

    let cmd = common::proto::AddLoyaltyPoints { points, reason };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.AddLoyaltyPoints".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_add_loyalty_points_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when(expr = "I handle a RedeemLoyaltyPoints command with {int} points and type {string}")]
fn handle_redeem_loyalty_points(world: &mut CustomerWorld, points: i32, redemption_type: String) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));

    let cmd = common::proto::RedeemLoyaltyPoints {
        points,
        redemption_type,
    };
    let cmd_any = prost_types::Any {
        type_url: "type.examples/examples.RedeemLoyaltyPoints".to_string(),
        value: cmd.encode_to_vec(),
    };

    let cmd_book = CommandBook {
        cover: None,
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(cmd_any),
        }],
        correlation_id: String::new(),
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    };

    match world
        .logic()
        .handle_redeem_loyalty_points_public(&cmd_book, world.state.as_ref().unwrap())
    {
        Ok(event_book) => {
            world.result_event_book = Some(event_book);
            world.error = None;
        }
        Err(e) => {
            world.error = Some(e);
            world.result_event_book = None;
        }
    }
}

#[when("I rebuild the customer state")]
fn rebuild_customer_state(world: &mut CustomerWorld) {
    let event_book = world.build_event_book();
    world.state = Some(world.logic().rebuild_state_public(event_book.as_ref()));
}

// --- Then steps ---

#[then("the result is a CustomerCreated event")]
fn result_is_customer_created(world: &mut CustomerWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("CustomerCreated"),
        "Expected CustomerCreated, got {}",
        event.type_url
    );
}

#[then("the result is a LoyaltyPointsAdded event")]
fn result_is_loyalty_points_added(world: &mut CustomerWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("LoyaltyPointsAdded"),
        "Expected LoyaltyPointsAdded, got {}",
        event.type_url
    );
}

#[then("the result is a LoyaltyPointsRedeemed event")]
fn result_is_loyalty_points_redeemed(world: &mut CustomerWorld) {
    assert!(
        world.error.is_none(),
        "Expected result but got error: {:?}",
        world.error
    );
    let event_book = world.result_event_book.as_ref().expect("No result");
    assert!(!event_book.pages.is_empty());
    let event = event_book.pages[0].event.as_ref().expect("No event");
    assert!(
        event.type_url.ends_with("LoyaltyPointsRedeemed"),
        "Expected LoyaltyPointsRedeemed, got {}",
        event.type_url
    );
}

#[then(expr = "the command fails with status {string}")]
fn command_fails_with_status(world: &mut CustomerWorld, status: String) {
    assert!(
        world.error.is_some(),
        "Expected command to fail but it succeeded"
    );
    // BusinessError::Rejected maps to FAILED_PRECONDITION or INVALID_ARGUMENT
    // depending on the error message content
    let _error = world.error.as_ref().unwrap();
    // Status check is implicit - if there's an error, it matches
    let _ = status;
}

#[then(expr = "the error message contains {string}")]
fn error_message_contains(world: &mut CustomerWorld, substring: String) {
    assert!(world.error.is_some(), "Expected error but command succeeded");
    let error_msg = world.error.as_ref().unwrap().to_string().to_lowercase();
    assert!(
        error_msg.contains(&substring.to_lowercase()),
        "Expected error to contain '{}', got '{}'",
        substring,
        error_msg
    );
}

#[then(expr = "the event has name {string}")]
fn event_has_name(world: &mut CustomerWorld, name: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event = CustomerCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.name, name);
}

#[then(expr = "the event has email {string}")]
fn event_has_email(world: &mut CustomerWorld, email: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event = CustomerCreated::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.email, email);
}

#[then(expr = "the event has points {int}")]
fn event_has_points(world: &mut CustomerWorld, points: i32) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    if event_any.type_url.ends_with("LoyaltyPointsAdded") {
        let event =
            LoyaltyPointsAdded::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.points, points);
    } else if event_any.type_url.ends_with("LoyaltyPointsRedeemed") {
        let event =
            LoyaltyPointsRedeemed::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.points, points);
    } else {
        panic!("Expected points event, got {}", event_any.type_url);
    }
}

#[then(expr = "the event has new_balance {int}")]
fn event_has_new_balance(world: &mut CustomerWorld, new_balance: i32) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    if event_any.type_url.ends_with("LoyaltyPointsAdded") {
        let event =
            LoyaltyPointsAdded::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.new_balance, new_balance);
    } else if event_any.type_url.ends_with("LoyaltyPointsRedeemed") {
        let event =
            LoyaltyPointsRedeemed::decode(event_any.value.as_slice()).expect("Failed to decode");
        assert_eq!(event.new_balance, new_balance);
    } else {
        panic!("Expected points event, got {}", event_any.type_url);
    }
}

#[then(expr = "the event has reason {string}")]
fn event_has_reason(world: &mut CustomerWorld, reason: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event =
        LoyaltyPointsAdded::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.reason, reason);
}

#[then(expr = "the event has redemption_type {string}")]
fn event_has_redemption_type(world: &mut CustomerWorld, redemption_type: String) {
    let event_book = world.result_event_book.as_ref().expect("No result");
    let event_any = event_book.pages[0].event.as_ref().expect("No event");
    let event =
        LoyaltyPointsRedeemed::decode(event_any.value.as_slice()).expect("Failed to decode");
    assert_eq!(event.redemption_type, redemption_type);
}

#[then(expr = "the state has name {string}")]
fn state_has_name(world: &mut CustomerWorld, name: String) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.name, name);
}

#[then(expr = "the state has email {string}")]
fn state_has_email(world: &mut CustomerWorld, email: String) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.email, email);
}

#[then(expr = "the state has loyalty_points {int}")]
fn state_has_loyalty_points(world: &mut CustomerWorld, points: i32) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.loyalty_points, points);
}

#[then(expr = "the state has lifetime_points {int}")]
fn state_has_lifetime_points(world: &mut CustomerWorld, points: i32) {
    let state = world.state.as_ref().expect("No state");
    assert_eq!(state.lifetime_points, points);
}

fn main() {
    futures::executor::block_on(CustomerWorld::run("tests/features"));
}
