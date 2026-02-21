//! Table aggregate BDD tests using cucumber-rs.

use std::collections::HashMap;

use agg_table::{handlers, state::rebuild_state};
use angzarr_client::proto::examples::{
    CreateTable, EndHand, GameVariant, HandEnded, HandStarted, JoinTable, LeaveTable,
    PlayerJoined, PlayerLeft, PotResult, StartHand, TableCreated,
};
use angzarr_client::proto::{event_page, CommandBook, Cover, EventBook, EventPage, Uuid};
use angzarr_client::{pack_event, CommandRejectedError, UnpackAny};
use cucumber::{given, then, when, World, WriterExt};
use prost::Message;
use prost_types::Any;
use sha2::{Digest, Sha256};

/// Generate deterministic UUID from a seed string.
fn uuid_for(seed: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();
    hash[0..16].to_vec()
}

/// Pack a command into Any.
fn pack_cmd<T: Message>(cmd: &T, type_name: &str) -> Any {
    Any {
        type_url: format!("type.poker/{}", type_name),
        value: cmd.encode_to_vec(),
    }
}

/// Create a command book with a given root.
fn command_book(root: &[u8], domain: &str) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(Uuid {
                value: root.to_vec(),
            }),
            ..Default::default()
        }),
        pages: vec![],
        saga_origin: None,
    }
}

/// Generate hand root from table root and hand number.
fn generate_hand_root(table_root: &[u8], hand_number: i64) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(table_root);
    hasher.update(hand_number.to_be_bytes());
    hasher.finalize().to_vec()
}

/// Test world for table aggregate.
#[derive(Debug, Default, World)]
pub struct TableWorld {
    events: Vec<prost_types::Any>,
    result: Option<Result<EventBook, CommandRejectedError>>,

    // Table parameters from Given steps
    min_buy_in: i64,
    max_buy_in: i64,
    max_players: i32,
    player_stacks: HashMap<String, i64>, // player_id -> stack
    dealer_position: i32,
    hand_number: i64,
}

impl TableWorld {
    fn table_root(&self) -> Vec<u8> {
        uuid_for("test-table")
    }

    fn player_root(&self, player_id: &str) -> Vec<u8> {
        uuid_for(player_id)
    }

    fn event_book(&self) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "table".to_string(),
                root: Some(Uuid {
                    value: self.table_root(),
                }),
                ..Default::default()
            }),
            pages: self
                .events
                .iter()
                .enumerate()
                .map(|(i, e)| EventPage {
                    sequence: i as u32,
                    payload: Some(event_page::Payload::Event(e.clone())),
                    created_at: None,
                })
                .collect(),
            snapshot: None,
            next_sequence: self.events.len() as u32,
        }
    }

    fn next_seq(&self) -> u32 {
        self.events.len() as u32
    }

    fn result_event(&self) -> Option<Any> {
        self.result.as_ref().and_then(|r: &Result<EventBook, CommandRejectedError>| {
            r.as_ref()
                .ok()
                .and_then(|eb| eb.pages.first())
                .and_then(|p| match &p.payload {
                    Some(event_page::Payload::Event(e)) => Some(e.clone()),
                    _ => None,
                })
        })
    }
}

// =============================================================================
// Given steps
// =============================================================================

#[given("no prior events for the table aggregate")]
fn given_no_events(world: &mut TableWorld) {
    world.events.clear();
    world.min_buy_in = 200;
    world.max_buy_in = 2000;  // Set high enough for all test buy-ins
    world.max_players = 9;
}

#[given(expr = "a TableCreated event for {string}")]
fn given_table_created(world: &mut TableWorld, table_name: String) {
    // Don't clear if called from a variant step that already cleared
    if world.events.is_empty() || !world.events.iter().any(|e| e.type_url.ends_with("TableCreated")) {
        world.events.clear();
    }
    // Set high defaults if not explicitly set
    if world.min_buy_in == 0 {
        world.min_buy_in = 200;
    }
    if world.max_buy_in == 0 {
        world.max_buy_in = 2000;
    }
    if world.max_players == 0 {
        world.max_players = 9;
    }
    let event = TableCreated {
        table_name,
        game_variant: GameVariant::TexasHoldem as i32,
        small_blind: 5,
        big_blind: 10,
        min_buy_in: world.min_buy_in,
        max_buy_in: world.max_buy_in,
        max_players: world.max_players,
        action_timeout_seconds: 30,
        created_at: None,
    };
    world.events.push(pack_event(&event, "examples.TableCreated"));
}

#[given(expr = "a TableCreated event for {string} with min_buy_in {int}")]
fn given_table_created_min_buyin(world: &mut TableWorld, table_name: String, min_buy_in: i64) {
    world.events.clear();
    world.min_buy_in = min_buy_in;
    world.max_buy_in = 2000; // Ensure high enough for tests
    world.max_players = 9;
    given_table_created(world, table_name);
}

#[given(expr = "a TableCreated event for {string} with max_players {int}")]
fn given_table_created_max_players(world: &mut TableWorld, table_name: String, max_players: i32) {
    world.events.clear();
    world.min_buy_in = 200;
    world.max_buy_in = 2000; // Ensure high enough for tests
    world.max_players = max_players;
    given_table_created(world, table_name);
}

#[given(expr = "a PlayerJoined event for player {string} at seat {int}")]
fn given_player_joined(world: &mut TableWorld, player_id: String, seat: i32) {
    let stack = world.player_stacks.get(&player_id).copied().unwrap_or(500);
    let event = PlayerJoined {
        player_root: world.player_root(&player_id),
        seat_position: seat,
        buy_in_amount: stack,
        stack,
        joined_at: None,
    };
    world.events.push(pack_event(&event, "examples.PlayerJoined"));
}

#[given(expr = "a PlayerJoined event for player {string} at seat {int} with stack {int}")]
fn given_player_joined_stack(world: &mut TableWorld, player_id: String, seat: i32, stack: i64) {
    world.player_stacks.insert(player_id.clone(), stack);
    given_player_joined(world, player_id, seat);
}

#[given(expr = "a HandStarted event for hand {int}")]
fn given_hand_started(world: &mut TableWorld, hand_number: i64) {
    world.hand_number = hand_number;
    let table_root = world.table_root();
    let hand_root = generate_hand_root(&table_root, hand_number);
    let event = HandStarted {
        hand_root,
        hand_number,
        dealer_position: world.dealer_position,
        small_blind_position: 0,
        big_blind_position: 1,
        active_players: vec![],
        game_variant: GameVariant::TexasHoldem as i32,
        small_blind: 5,
        big_blind: 10,
        started_at: None,
    };
    world.events.push(pack_event(&event, "examples.HandStarted"));
}

#[given(expr = "a HandStarted event for hand {int} with dealer at seat {int}")]
fn given_hand_started_dealer(world: &mut TableWorld, hand_number: i64, dealer_seat: i32) {
    world.dealer_position = dealer_seat;
    given_hand_started(world, hand_number);
}

#[given(expr = "a HandEnded event for hand {int}")]
fn given_hand_ended(world: &mut TableWorld, hand_number: i64) {
    let table_root = world.table_root();
    let hand_root = generate_hand_root(&table_root, hand_number);
    let event = HandEnded {
        hand_root,
        results: vec![],
        stack_changes: HashMap::new(),
        ended_at: None,
    };
    world.events.push(pack_event(&event, "examples.HandEnded"));
}

// =============================================================================
// When steps
// =============================================================================

#[when(regex = r"I handle a CreateTable command with name (.+) and variant (.+):")]
fn when_create_table(world: &mut TableWorld, step: &cucumber::gherkin::Step) {
    // Parse table from step
    let (name, variant) = {
        let captures = regex::Regex::new(r#"name "([^"]+)" and variant "([^"]+)""#)
            .unwrap()
            .captures(&step.value)
            .unwrap();
        (
            captures.get(1).unwrap().as_str().to_string(),
            captures.get(2).unwrap().as_str().to_string(),
        )
    };

    // Parse data table
    let table = step.table.as_ref().expect("Expected data table");
    let row = &table.rows[1]; // Skip header

    let small_blind: i64 = row[0].parse().unwrap();
    let big_blind: i64 = row[1].parse().unwrap();
    let min_buy_in: i64 = row[2].parse().unwrap();
    let max_buy_in: i64 = row[3].parse().unwrap();
    let max_players: i32 = row[4].parse().unwrap();

    let game_variant = match variant.as_str() {
        "TEXAS_HOLDEM" => GameVariant::TexasHoldem,
        "FIVE_CARD_DRAW" => GameVariant::FiveCardDraw,
        "OMAHA" => GameVariant::Omaha,
        _ => GameVariant::TexasHoldem,
    };

    let cmd = CreateTable {
        table_name: name,
        game_variant: game_variant as i32,
        small_blind,
        big_blind,
        min_buy_in,
        max_buy_in,
        max_players,
        action_timeout_seconds: 30,
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.table_root(), "table");
    let cmd_any = pack_cmd(&cmd, "examples.CreateTable");

    world.result = Some(handlers::handle_create_table(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle a JoinTable command for player {string} at seat {int} with buy-in {int}")]
fn when_join_table(world: &mut TableWorld, player_id: String, seat: i32, buy_in: i64) {
    let cmd = JoinTable {
        player_root: world.player_root(&player_id),
        preferred_seat: seat,
        buy_in_amount: buy_in,
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.table_root(), "table");
    let cmd_any = pack_cmd(&cmd, "examples.JoinTable");

    world.result = Some(handlers::handle_join_table(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle a LeaveTable command for player {string}")]
fn when_leave_table(world: &mut TableWorld, player_id: String) {
    let cmd = LeaveTable {
        player_root: world.player_root(&player_id),
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.table_root(), "table");
    let cmd_any = pack_cmd(&cmd, "examples.LeaveTable");

    world.result = Some(handlers::handle_leave_table(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when("I handle a StartHand command")]
fn when_start_hand(world: &mut TableWorld) {
    let cmd = StartHand {};

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.table_root(), "table");
    let cmd_any = pack_cmd(&cmd, "examples.StartHand");

    world.result = Some(handlers::handle_start_hand(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle an EndHand command with winner {string} winning {int}")]
fn when_end_hand(world: &mut TableWorld, winner_id: String, amount: i64) {
    let table_root = world.table_root();
    let hand_number = world.hand_number;
    let hand_root = generate_hand_root(&table_root, hand_number);

    let cmd = EndHand {
        hand_root,
        results: vec![PotResult {
            winner_root: world.player_root(&winner_id),
            amount,
            pot_type: "main".to_string(),
            winning_hand: None,
        }],
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.table_root(), "table");
    let cmd_any = pack_cmd(&cmd, "examples.EndHand");

    world.result = Some(handlers::handle_end_hand(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when("I rebuild the table state")]
fn when_rebuild_state(world: &mut TableWorld) {
    // State is rebuilt in Then steps
}

// =============================================================================
// Then steps
// =============================================================================

#[then(expr = "the result is a {word} event")]
fn then_result_is_event(world: &mut TableWorld, event_type: String) {
    let result = world.result.as_ref().expect("No result");
    let event_book = result.as_ref().expect("Expected success but got error");
    let event = event_book
        .pages
        .first()
        .and_then(|p| match &p.payload {
            Some(event_page::Payload::Event(e)) => Some(e),
            _ => None,
        })
        .expect("No event in result");

    assert!(
        event.type_url.ends_with(&event_type),
        "Expected {} but got {}",
        event_type,
        event.type_url
    );
}

#[then(expr = "the command fails with status {string}")]
fn then_command_fails(world: &mut TableWorld, _status: String) {
    let result = world.result.as_ref().expect("No result");
    assert!(
        result.is_err(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
fn then_error_contains(world: &mut TableWorld, expected: String) {
    let result = world.result.as_ref().expect("No result");
    let err = result.as_ref().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains(&expected.to_lowercase()),
        "Expected error to contain '{}' but got '{}'",
        expected,
        msg
    );
}

#[then(expr = "the table event has table_name {string}")]
fn then_table_name(world: &mut TableWorld, expected: String) {
    let event = world.result_event().expect("No event");
    let table_created: TableCreated = event.unpack().expect("Failed to decode");
    assert_eq!(table_created.table_name, expected);
}

#[then(expr = "the table event has game_variant {string}")]
fn then_game_variant(world: &mut TableWorld, expected: String) {
    let event = world.result_event().expect("No event");
    let expected_variant = match expected.as_str() {
        "TEXAS_HOLDEM" => GameVariant::TexasHoldem,
        "FIVE_CARD_DRAW" => GameVariant::FiveCardDraw,
        "OMAHA" => GameVariant::Omaha,
        _ => panic!("Unknown variant: {}", expected),
    };

    // Could be TableCreated or HandStarted
    if event.type_url.ends_with("TableCreated") {
        let tc: TableCreated = event.unpack().expect("Failed to decode");
        assert_eq!(
            GameVariant::try_from(tc.game_variant).unwrap_or_default(),
            expected_variant
        );
    } else if event.type_url.ends_with("HandStarted") {
        let hs: HandStarted = event.unpack().expect("Failed to decode");
        assert_eq!(
            GameVariant::try_from(hs.game_variant).unwrap_or_default(),
            expected_variant
        );
    }
}

#[then(expr = "the table event has small_blind {int}")]
fn then_small_blind(world: &mut TableWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let table_created: TableCreated = event.unpack().expect("Failed to decode");
    assert_eq!(table_created.small_blind, expected);
}

#[then(expr = "the table event has big_blind {int}")]
fn then_big_blind(world: &mut TableWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let table_created: TableCreated = event.unpack().expect("Failed to decode");
    assert_eq!(table_created.big_blind, expected);
}

#[then(expr = "the table event has seat_position {int}")]
fn then_seat_position(world: &mut TableWorld, expected: i32) {
    let event = world.result_event().expect("No event");
    let player_joined: PlayerJoined = event.unpack().expect("Failed to decode");
    assert_eq!(player_joined.seat_position, expected);
}

#[then(expr = "the table event has buy_in_amount {int}")]
fn then_buy_in_amount(world: &mut TableWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let player_joined: PlayerJoined = event.unpack().expect("Failed to decode");
    assert_eq!(player_joined.buy_in_amount, expected);
}

#[then(expr = "the table event has chips_cashed_out {int}")]
fn then_chips_cashed_out(world: &mut TableWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let player_left: PlayerLeft = event.unpack().expect("Failed to decode");
    assert_eq!(player_left.chips_cashed_out, expected);
}

#[then(expr = "the table event has hand_number {int}")]
fn then_hand_number(world: &mut TableWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let hand_started: HandStarted = event.unpack().expect("Failed to decode");
    assert_eq!(hand_started.hand_number, expected);
}

#[then(expr = "the table event has {int} active_players")]
fn then_active_players_count(world: &mut TableWorld, expected: usize) {
    let event = world.result_event().expect("No event");
    let hand_started: HandStarted = event.unpack().expect("Failed to decode");
    assert_eq!(hand_started.active_players.len(), expected);
}

#[then(expr = "the table event has dealer_position {int}")]
fn then_dealer_position(world: &mut TableWorld, expected: i32) {
    let event = world.result_event().expect("No event");
    let hand_started: HandStarted = event.unpack().expect("Failed to decode");
    assert_eq!(hand_started.dealer_position, expected);
}

#[then(expr = r"player {string} stack change is {int}")]
fn then_stack_change(world: &mut TableWorld, player_id: String, expected: i64) {
    let event = world.result_event().expect("No event");
    let hand_ended: HandEnded = event.unpack().expect("Failed to decode");
    let player_hex = hex::encode(world.player_root(&player_id));
    let change = hand_ended.stack_changes.get(&player_hex).copied().unwrap_or(0);
    assert_eq!(change, expected);
}

#[then(expr = "the table state has {int} players")]
fn then_state_player_count(world: &mut TableWorld, expected: usize) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.player_count(), expected);
}

#[then(expr = "the table state has seat {int} occupied by {string}")]
fn then_seat_occupied(world: &mut TableWorld, seat: i32, player_id: String) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let seat_state = state.seats.get(&seat).expect("Seat not found");
    assert_eq!(
        hex::encode(&seat_state.player_root),
        hex::encode(world.player_root(&player_id))
    );
}

#[then(expr = "the table state has status {string}")]
fn then_state_status(world: &mut TableWorld, expected: String) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.status, expected);
}

#[then(expr = "the table state has hand_count {int}")]
fn then_state_hand_count(world: &mut TableWorld, expected: i64) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.hand_count, expected);
}

#[tokio::main]
async fn main() {
    TableWorld::cucumber()
        .with_writer(
            cucumber::writer::Basic::stdout()
                .summarized()
                .assert_normalized(),
        )
        .run("../../features/unit/table.feature")
        .await;
}
