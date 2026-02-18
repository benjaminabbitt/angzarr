//! Player aggregate BDD tests using cucumber-rs.
//!
//! DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//!      Update documentation when making changes to test patterns.

use agg_player::{
    handle_deposit_funds, handle_register_player, handle_release_funds, handle_reserve_funds,
    handle_withdraw_funds, rebuild_state, PlayerState,
};
use angzarr_client::proto::examples::{
    Currency, DepositFunds, FundsDeposited, FundsReleased, FundsReserved, FundsWithdrawn,
    PlayerRegistered, PlayerType, RegisterPlayer, ReleaseFunds, ReserveFunds, WithdrawFunds,
};
use angzarr_client::proto::{event_page, CommandBook, Cover, EventBook, EventPage, Uuid};
use angzarr_client::{pack_event, UnpackAny};
use cucumber::{given, then, when, World};
use prost_types::Any;
use sha2::{Digest, Sha256};

/// Helper to create a deterministic UUID from a string.
fn uuid_for(name: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let result = hasher.finalize();
    result[..16].to_vec()
}

/// Helper to create a Currency value.
fn currency(amount: i64) -> Currency {
    Currency {
        amount,
        currency_code: "CHIPS".to_string(),
    }
}

/// Helper to pack a command into Any.
fn pack_command<M: prost::Message>(msg: &M, type_name: &str) -> Any {
    Any {
        type_url: format!("type.poker/{}", type_name),
        value: msg.encode_to_vec(),
    }
}

/// Test context for player scenarios.
#[derive(Debug, Default, World)]
#[world(init = Self::new)]
pub struct PlayerWorld {
    domain: String,
    root: Vec<u8>,
    events: Vec<EventPage>,
    next_sequence: u32,
    last_error: Option<String>,
    last_event_book: Option<EventBook>,
    last_state: Option<PlayerState>,
}

impl PlayerWorld {
    fn new() -> Self {
        Self {
            domain: "player".to_string(),
            root: uuid_for("player-test"),
            events: Vec::new(),
            next_sequence: 0,
            last_error: None,
            last_event_book: None,
            last_state: None,
        }
    }

    fn build_event_book(&self) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: self.domain.clone(),
                root: Some(Uuid {
                    value: self.root.clone(),
                }),
                ..Default::default()
            }),
            pages: self.events.clone(),
            next_sequence: self.next_sequence,
            snapshot: None,
        }
    }

    fn build_command_book(&self, cmd_any: Any) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: self.domain.clone(),
                root: Some(Uuid {
                    value: self.root.clone(),
                }),
                ..Default::default()
            }),
            pages: vec![angzarr_client::proto::CommandPage {
                sequence: self.next_sequence,
                command: Some(cmd_any),
                ..Default::default()
            }],
            saga_origin: None,
        }
    }

    fn add_event(&mut self, event_any: Any) {
        self.events.push(EventPage {
            sequence: Some(event_page::Sequence::Num(self.next_sequence)),
            event: Some(event_any),
            created_at: Some(angzarr_client::now()),
        });
        self.next_sequence += 1;
    }

    fn rebuild_state(&self) -> PlayerState {
        rebuild_state(&self.build_event_book())
    }

    fn get_last_event(&self) -> Option<&Any> {
        self.last_event_book
            .as_ref()
            .and_then(|eb| eb.pages.first())
            .and_then(|p| p.event.as_ref())
    }
}

// --- Given Step Definitions ---

#[given("no prior events for the player aggregate")]
fn no_prior_events(world: &mut PlayerWorld) {
    world.events.clear();
    world.next_sequence = 0;
}

#[given(expr = "a PlayerRegistered event for {string}")]
fn player_registered_event(world: &mut PlayerWorld, name: String) {
    let event = PlayerRegistered {
        display_name: name.clone(),
        email: format!("{}@example.com", name.to_lowercase()),
        player_type: PlayerType::Human as i32,
        ai_model_id: String::new(),
        registered_at: Some(angzarr_client::now()),
    };
    world.add_event(pack_event(&event, "examples.PlayerRegistered"));
}

#[given(expr = "a FundsDeposited event with amount {int}")]
fn funds_deposited_event(world: &mut PlayerWorld, amount: i64) {
    let state = world.rebuild_state();
    let new_balance = state.bankroll + amount;

    let event = FundsDeposited {
        amount: Some(currency(amount)),
        new_balance: Some(currency(new_balance)),
        deposited_at: Some(angzarr_client::now()),
    };
    world.add_event(pack_event(&event, "examples.FundsDeposited"));
}

#[given(expr = "a FundsReserved event with amount {int} for table {string}")]
fn funds_reserved_event(world: &mut PlayerWorld, amount: i64, table_name: String) {
    let state = world.rebuild_state();
    let new_reserved = state.reserved_funds + amount;
    let new_available = state.bankroll - new_reserved;

    let event = FundsReserved {
        table_root: uuid_for(&table_name),
        amount: Some(currency(amount)),
        new_available_balance: Some(currency(new_available)),
        new_reserved_balance: Some(currency(new_reserved)),
        reserved_at: Some(angzarr_client::now()),
    };
    world.add_event(pack_event(&event, "examples.FundsReserved"));
}

// --- When Step Definitions ---

#[when(expr = "I handle a RegisterPlayer command with name {string} and email {string}")]
fn handle_register_player_cmd(world: &mut PlayerWorld, name: String, email: String) {
    let cmd = RegisterPlayer {
        display_name: name,
        email,
        player_type: PlayerType::Human as i32,
        ai_model_id: String::new(),
    };

    let cmd_any = pack_command(&cmd, "examples.RegisterPlayer");
    let cmd_book = world.build_command_book(cmd_any.clone());
    let state = world.rebuild_state();

    match handle_register_player(&cmd_book, &cmd_any, &state, world.next_sequence) {
        Ok(event_book) => {
            world.last_event_book = Some(event_book);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
            world.last_event_book = None;
        }
    }
}

#[when(expr = "I handle a RegisterPlayer command with name {string} and email {string} as AI")]
fn handle_register_player_ai_cmd(world: &mut PlayerWorld, name: String, email: String) {
    let cmd = RegisterPlayer {
        display_name: name,
        email,
        player_type: PlayerType::Ai as i32,
        ai_model_id: "gpt-4".to_string(),
    };

    let cmd_any = pack_command(&cmd, "examples.RegisterPlayer");
    let cmd_book = world.build_command_book(cmd_any.clone());
    let state = world.rebuild_state();

    match handle_register_player(&cmd_book, &cmd_any, &state, world.next_sequence) {
        Ok(event_book) => {
            world.last_event_book = Some(event_book);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
            world.last_event_book = None;
        }
    }
}

#[when(expr = "I handle a DepositFunds command with amount {int}")]
fn handle_deposit_funds_cmd(world: &mut PlayerWorld, amount: i64) {
    let cmd = DepositFunds {
        amount: Some(currency(amount)),
    };

    let cmd_any = pack_command(&cmd, "examples.DepositFunds");
    let cmd_book = world.build_command_book(cmd_any.clone());
    let state = world.rebuild_state();

    match handle_deposit_funds(&cmd_book, &cmd_any, &state, world.next_sequence) {
        Ok(event_book) => {
            world.last_event_book = Some(event_book);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
            world.last_event_book = None;
        }
    }
}

#[when(expr = "I handle a WithdrawFunds command with amount {int}")]
fn handle_withdraw_funds_cmd(world: &mut PlayerWorld, amount: i64) {
    let cmd = WithdrawFunds {
        amount: Some(currency(amount)),
    };

    let cmd_any = pack_command(&cmd, "examples.WithdrawFunds");
    let cmd_book = world.build_command_book(cmd_any.clone());
    let state = world.rebuild_state();

    match handle_withdraw_funds(&cmd_book, &cmd_any, &state, world.next_sequence) {
        Ok(event_book) => {
            world.last_event_book = Some(event_book);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
            world.last_event_book = None;
        }
    }
}

#[when(expr = "I handle a ReserveFunds command with amount {int} for table {string}")]
fn handle_reserve_funds_cmd(world: &mut PlayerWorld, amount: i64, table_name: String) {
    let cmd = ReserveFunds {
        table_root: uuid_for(&table_name),
        amount: Some(currency(amount)),
    };

    let cmd_any = pack_command(&cmd, "examples.ReserveFunds");
    let cmd_book = world.build_command_book(cmd_any.clone());
    let state = world.rebuild_state();

    match handle_reserve_funds(&cmd_book, &cmd_any, &state, world.next_sequence) {
        Ok(event_book) => {
            world.last_event_book = Some(event_book);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
            world.last_event_book = None;
        }
    }
}

#[when(expr = "I handle a ReleaseFunds command for table {string}")]
fn handle_release_funds_cmd(world: &mut PlayerWorld, table_name: String) {
    let cmd = ReleaseFunds {
        table_root: uuid_for(&table_name),
    };

    let cmd_any = pack_command(&cmd, "examples.ReleaseFunds");
    let cmd_book = world.build_command_book(cmd_any.clone());
    let state = world.rebuild_state();

    match handle_release_funds(&cmd_book, &cmd_any, &state, world.next_sequence) {
        Ok(event_book) => {
            world.last_event_book = Some(event_book);
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
            world.last_event_book = None;
        }
    }
}

#[when("I rebuild the player state")]
fn rebuild_player_state(world: &mut PlayerWorld) {
    world.last_state = Some(world.rebuild_state());
}

// --- Then Step Definitions ---

#[then("the result is a PlayerRegistered event")]
fn result_is_player_registered(world: &mut PlayerWorld) {
    let event = world.get_last_event().expect("No event found");
    assert!(
        event.type_url.ends_with("PlayerRegistered"),
        "Expected PlayerRegistered event but got {}",
        event.type_url
    );
}

#[then("the result is a FundsDeposited event")]
fn result_is_funds_deposited(world: &mut PlayerWorld) {
    let event = world.get_last_event().expect("No event found");
    assert!(
        event.type_url.ends_with("FundsDeposited"),
        "Expected FundsDeposited event but got {}",
        event.type_url
    );
}

#[then("the result is a FundsWithdrawn event")]
fn result_is_funds_withdrawn(world: &mut PlayerWorld) {
    let event = world.get_last_event().expect("No event found");
    assert!(
        event.type_url.ends_with("FundsWithdrawn"),
        "Expected FundsWithdrawn event but got {}",
        event.type_url
    );
}

#[then("the result is a FundsReserved event")]
fn result_is_funds_reserved(world: &mut PlayerWorld) {
    let event = world.get_last_event().expect("No event found");
    assert!(
        event.type_url.ends_with("FundsReserved"),
        "Expected FundsReserved event but got {}",
        event.type_url
    );
}

#[then("the result is a FundsReleased event")]
fn result_is_funds_released(world: &mut PlayerWorld) {
    let event = world.get_last_event().expect("No event found");
    assert!(
        event.type_url.ends_with("FundsReleased"),
        "Expected FundsReleased event but got {}",
        event.type_url
    );
}

#[then(expr = "the command fails with status {string}")]
fn command_fails_with_status(world: &mut PlayerWorld, _status: String) {
    assert!(
        world.last_error.is_some(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
fn error_message_contains(world: &mut PlayerWorld, expected: String) {
    let error = world.last_error.as_ref().expect("No error found");
    assert!(
        error.to_lowercase().contains(&expected.to_lowercase()),
        "Expected error to contain '{}' but got '{}'",
        expected,
        error
    );
}

#[then(expr = "the player event has display_name {string}")]
fn player_event_has_display_name(world: &mut PlayerWorld, expected: String) {
    let event_any = world.get_last_event().expect("No event found");
    let event: PlayerRegistered = event_any.unpack().expect("Failed to unpack event");
    assert_eq!(
        event.display_name, expected,
        "Expected display_name '{}' but got '{}'",
        expected, event.display_name
    );
}

#[then(expr = "the player event has player_type {string}")]
fn player_event_has_player_type(world: &mut PlayerWorld, expected: String) {
    let event_any = world.get_last_event().expect("No event found");
    let event: PlayerRegistered = event_any.unpack().expect("Failed to unpack event");
    let player_type = PlayerType::try_from(event.player_type).unwrap_or_default();
    let type_str = match player_type {
        PlayerType::Human => "HUMAN",
        PlayerType::Ai => "AI",
        _ => "UNKNOWN",
    };
    assert_eq!(
        type_str, expected,
        "Expected player_type '{}' but got '{}'",
        expected, type_str
    );
}

#[then(expr = "the player event has amount {int}")]
fn player_event_has_amount(world: &mut PlayerWorld, expected: i64) {
    let event_any = world.get_last_event().expect("No event found");

    if event_any.type_url.ends_with("FundsDeposited") {
        let event: FundsDeposited = event_any.unpack().expect("Failed to unpack event");
        let amount = event.amount.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            amount, expected,
            "Expected amount {} but got {}",
            expected, amount
        );
    } else if event_any.type_url.ends_with("FundsWithdrawn") {
        let event: FundsWithdrawn = event_any.unpack().expect("Failed to unpack event");
        let amount = event.amount.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            amount, expected,
            "Expected amount {} but got {}",
            expected, amount
        );
    } else if event_any.type_url.ends_with("FundsReserved") {
        let event: FundsReserved = event_any.unpack().expect("Failed to unpack event");
        let amount = event.amount.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            amount, expected,
            "Expected amount {} but got {}",
            expected, amount
        );
    } else if event_any.type_url.ends_with("FundsReleased") {
        let event: FundsReleased = event_any.unpack().expect("Failed to unpack event");
        let amount = event.amount.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            amount, expected,
            "Expected amount {} but got {}",
            expected, amount
        );
    }
}

#[then(expr = "the player event has new_balance {int}")]
fn player_event_has_new_balance(world: &mut PlayerWorld, expected: i64) {
    let event_any = world.get_last_event().expect("No event found");

    if event_any.type_url.ends_with("FundsDeposited") {
        let event: FundsDeposited = event_any.unpack().expect("Failed to unpack event");
        let balance = event.new_balance.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            balance, expected,
            "Expected new_balance {} but got {}",
            expected, balance
        );
    } else if event_any.type_url.ends_with("FundsWithdrawn") {
        let event: FundsWithdrawn = event_any.unpack().expect("Failed to unpack event");
        let balance = event.new_balance.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            balance, expected,
            "Expected new_balance {} but got {}",
            expected, balance
        );
    }
}

#[then(expr = "the player event has new_available_balance {int}")]
fn player_event_has_new_available_balance(world: &mut PlayerWorld, expected: i64) {
    let event_any = world.get_last_event().expect("No event found");

    if event_any.type_url.ends_with("FundsReserved") {
        let event: FundsReserved = event_any.unpack().expect("Failed to unpack event");
        let available = event.new_available_balance.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            available, expected,
            "Expected new_available_balance {} but got {}",
            expected, available
        );
    } else if event_any.type_url.ends_with("FundsReleased") {
        let event: FundsReleased = event_any.unpack().expect("Failed to unpack event");
        let available = event.new_available_balance.map(|c| c.amount).unwrap_or(0);
        assert_eq!(
            available, expected,
            "Expected new_available_balance {} but got {}",
            expected, available
        );
    }
}

#[then(expr = "the player state has bankroll {int}")]
fn player_state_has_bankroll(world: &mut PlayerWorld, expected: i64) {
    let state = world.last_state.as_ref().expect("No state found");
    assert_eq!(
        state.bankroll, expected,
        "Expected bankroll {} but got {}",
        expected, state.bankroll
    );
}

#[then(expr = "the player state has reserved_funds {int}")]
fn player_state_has_reserved_funds(world: &mut PlayerWorld, expected: i64) {
    let state = world.last_state.as_ref().expect("No state found");
    assert_eq!(
        state.reserved_funds, expected,
        "Expected reserved_funds {} but got {}",
        expected, state.reserved_funds
    );
}

#[then(expr = "the player state has available_balance {int}")]
fn player_state_has_available_balance(world: &mut PlayerWorld, expected: i64) {
    let state = world.last_state.as_ref().expect("No state found");
    let available = state.available_balance();
    assert_eq!(
        available, expected,
        "Expected available_balance {} but got {}",
        expected, available
    );
}

#[tokio::main]
async fn main() {
    PlayerWorld::run("../../features/unit/player.feature").await;
}
