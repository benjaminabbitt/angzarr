//! Table Aggregate using OO-style proc macros.
//!
//! DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//!      Update documentation when making changes to handler patterns.
//!
//! This example demonstrates the OO pattern using:
//! - `#[aggregate(domain = "...")]` on impl blocks
//! - `#[handles(CommandType)]` on handler methods
//! - `#[applies(EventType)]` on event applier methods

use std::collections::HashMap;

use angzarr_client::proto::examples::{
    CreateTable, EndHand, GameVariant, HandEnded, HandStarted, JoinTable, LeaveTable,
    PlayerJoined, PlayerLeft, SeatSnapshot, StartHand, TableCreated,
};
use angzarr_client::proto::{CommandBook, EventBook, EventPage, event_page};
use angzarr_client::{run_aggregate_server, CommandRejectedError, CommandResult};
#[allow(unused_imports)]
use angzarr_macros::{aggregate, applies, handles};
use prost_types::Any;
use sha2::{Digest, Sha256};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Seat state at the table.
#[derive(Debug, Clone, Default)]
pub struct SeatState {
    pub position: i32,
    pub player_root: Vec<u8>,
    pub stack: i64,
    pub is_sitting_out: bool,
}

/// Table aggregate state.
#[derive(Debug, Default, Clone)]
pub struct TableState {
    pub table_id: String,
    pub table_name: String,
    pub game_variant: GameVariant,
    pub small_blind: i64,
    pub big_blind: i64,
    pub min_buy_in: i64,
    pub max_buy_in: i64,
    pub max_players: i32,
    pub action_timeout_seconds: i32,
    pub seats: HashMap<i32, SeatState>,
    pub dealer_position: i32,
    pub hand_count: i64,
    pub current_hand_root: Vec<u8>,
    pub status: String,
}

impl TableState {
    pub fn exists(&self) -> bool {
        !self.table_id.is_empty()
    }

    pub fn active_player_count(&self) -> usize {
        self.seats.values().filter(|s| !s.is_sitting_out).count()
    }

    pub fn find_seat_by_player(&self, player_root: &[u8]) -> Option<i32> {
        let player_hex = hex::encode(player_root);
        self.seats.iter().find_map(|(pos, seat)| {
            if hex::encode(&seat.player_root) == player_hex {
                Some(*pos)
            } else {
                None
            }
        })
    }

    pub fn next_available_seat(&self) -> Option<i32> {
        for i in 0..self.max_players {
            if !self.seats.contains_key(&i) {
                return Some(i);
            }
        }
        None
    }
}

/// Table aggregate using OO-style annotations.
pub struct TableAggregate;

// docs:start:oo_handlers
#[aggregate(domain = "table", state = TableState)]
impl TableAggregate {
    // ==========================================================================
    // Event Appliers
    // ==========================================================================

    #[applies(TableCreated)]
    fn apply_created(state: &mut TableState, event: TableCreated) {
        state.table_id = format!("table_{}", event.table_name);
        state.table_name = event.table_name;
        state.game_variant = GameVariant::try_from(event.game_variant).unwrap_or_default();
        state.small_blind = event.small_blind;
        state.big_blind = event.big_blind;
        state.min_buy_in = event.min_buy_in;
        state.max_buy_in = event.max_buy_in;
        state.max_players = event.max_players;
        state.action_timeout_seconds = event.action_timeout_seconds;
        state.status = "waiting".to_string();
    }

    #[applies(PlayerJoined)]
    fn apply_player_joined(state: &mut TableState, event: PlayerJoined) {
        state.seats.insert(
            event.seat_position,
            SeatState {
                position: event.seat_position,
                player_root: event.player_root,
                stack: event.stack,
                is_sitting_out: false,
            },
        );
    }

    #[applies(PlayerLeft)]
    fn apply_player_left(state: &mut TableState, event: PlayerLeft) {
        state.seats.remove(&event.seat_position);
    }

    #[applies(HandStarted)]
    fn apply_hand_started(state: &mut TableState, event: HandStarted) {
        state.current_hand_root = event.hand_root;
        state.hand_count = event.hand_number;
        state.dealer_position = event.dealer_position;
        state.status = "in_hand".to_string();
    }

    #[applies(HandEnded)]
    fn apply_hand_ended(state: &mut TableState, event: HandEnded) {
        state.current_hand_root.clear();
        state.status = "waiting".to_string();
        for (player_hex, delta) in &event.stack_changes {
            for seat in state.seats.values_mut() {
                if hex::encode(&seat.player_root) == *player_hex {
                    seat.stack += delta;
                    break;
                }
            }
        }
    }

    // ==========================================================================
    // Command Handlers
    // ==========================================================================

    #[handles(CreateTable)]
    pub fn create(
        &self,
        cb: &CommandBook,
        cmd: CreateTable,
        state: &TableState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        // Guard
        if state.exists() {
            return Err(CommandRejectedError::new("Table already exists"));
        }

        // Validate
        if cmd.table_name.is_empty() {
            return Err(CommandRejectedError::new("table_name is required"));
        }
        if cmd.small_blind <= 0 {
            return Err(CommandRejectedError::new("small_blind must be positive"));
        }
        if cmd.big_blind < cmd.small_blind {
            return Err(CommandRejectedError::new("big_blind must be >= small_blind"));
        }
        if cmd.max_players < 2 || cmd.max_players > 10 {
            return Err(CommandRejectedError::new("max_players must be 2-10"));
        }

        // Compute
        let event = TableCreated {
            table_name: cmd.table_name,
            game_variant: cmd.game_variant,
            small_blind: cmd.small_blind,
            big_blind: cmd.big_blind,
            min_buy_in: cmd.min_buy_in,
            max_buy_in: cmd.max_buy_in,
            max_players: cmd.max_players,
            action_timeout_seconds: cmd.action_timeout_seconds,
            created_at: Some(angzarr_client::now()),
        };

        Ok(new_event_book(cb, seq, &event, "examples.TableCreated"))
    }
    // docs:end:oo_handlers

    #[handles(JoinTable)]
    pub fn join(
        &self,
        cb: &CommandBook,
        cmd: JoinTable,
        state: &TableState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        // Guard
        if !state.exists() {
            return Err(CommandRejectedError::new("Table does not exist"));
        }

        // Validate
        if cmd.player_root.is_empty() {
            return Err(CommandRejectedError::new("player_root is required"));
        }
        if state.find_seat_by_player(&cmd.player_root).is_some() {
            return Err(CommandRejectedError::new("Player already seated"));
        }
        if cmd.buy_in_amount < state.min_buy_in {
            return Err(CommandRejectedError::new("Buy-in below minimum"));
        }
        if cmd.buy_in_amount > state.max_buy_in {
            return Err(CommandRejectedError::new("Buy-in above maximum"));
        }

        let seat_position = if cmd.preferred_seat >= 0 && cmd.preferred_seat < state.max_players {
            if state.seats.contains_key(&cmd.preferred_seat) {
                return Err(CommandRejectedError::new("Seat is occupied"));
            }
            cmd.preferred_seat
        } else {
            state
                .next_available_seat()
                .ok_or_else(|| CommandRejectedError::new("Table is full"))?
        };

        // Compute
        let event = PlayerJoined {
            player_root: cmd.player_root,
            seat_position,
            buy_in_amount: cmd.buy_in_amount,
            stack: cmd.buy_in_amount,
            joined_at: Some(angzarr_client::now()),
        };

        Ok(new_event_book(cb, seq, &event, "examples.PlayerJoined"))
    }

    #[handles(LeaveTable)]
    pub fn leave(
        &self,
        cb: &CommandBook,
        cmd: LeaveTable,
        state: &TableState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        // Guard
        if !state.exists() {
            return Err(CommandRejectedError::new("Table does not exist"));
        }
        if state.status == "in_hand" {
            return Err(CommandRejectedError::new("Cannot leave during a hand"));
        }

        // Validate
        let seat_position = state
            .find_seat_by_player(&cmd.player_root)
            .ok_or_else(|| CommandRejectedError::new("Player not at table"))?;

        // Compute
        let seat = state.seats.get(&seat_position).unwrap();
        let event = PlayerLeft {
            player_root: cmd.player_root,
            seat_position,
            chips_cashed_out: seat.stack,
            left_at: Some(angzarr_client::now()),
        };

        Ok(new_event_book(cb, seq, &event, "examples.PlayerLeft"))
    }

    #[handles(StartHand)]
    pub fn start_hand(
        &self,
        cb: &CommandBook,
        _cmd: StartHand,
        state: &TableState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        // Guard
        if !state.exists() {
            return Err(CommandRejectedError::new("Table does not exist"));
        }
        if state.status == "in_hand" {
            return Err(CommandRejectedError::new("Hand already in progress"));
        }
        if state.active_player_count() < 2 {
            return Err(CommandRejectedError::new("Not enough players"));
        }

        let table_root = cb
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|u| u.value.as_slice())
            .unwrap_or(&[]);

        // Compute
        let hand_number = state.hand_count + 1;
        let hand_root = generate_hand_root(table_root, hand_number);
        let dealer_position = advance_to_next_active(state.dealer_position, state);
        let small_blind_position = advance_to_next_active(dealer_position, state);
        let big_blind_position = advance_to_next_active(small_blind_position, state);

        let active_players: Vec<SeatSnapshot> = state
            .seats
            .values()
            .filter(|seat| !seat.is_sitting_out)
            .map(|seat| SeatSnapshot {
                position: seat.position,
                player_root: seat.player_root.clone(),
                stack: seat.stack,
            })
            .collect();

        let event = HandStarted {
            hand_root,
            hand_number,
            dealer_position,
            small_blind_position,
            big_blind_position,
            active_players,
            game_variant: state.game_variant as i32,
            small_blind: state.small_blind,
            big_blind: state.big_blind,
            started_at: Some(angzarr_client::now()),
        };

        Ok(new_event_book(cb, seq, &event, "examples.HandStarted"))
    }

    #[handles(EndHand)]
    pub fn end_hand(
        &self,
        cb: &CommandBook,
        cmd: EndHand,
        state: &TableState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        // Guard
        if !state.exists() {
            return Err(CommandRejectedError::new("Table does not exist"));
        }
        if state.status != "in_hand" {
            return Err(CommandRejectedError::new("No hand in progress"));
        }
        if hex::encode(&cmd.hand_root) != hex::encode(&state.current_hand_root) {
            return Err(CommandRejectedError::new("Hand root mismatch"));
        }

        // Compute stack changes from results
        let mut stack_changes: HashMap<String, i64> = HashMap::new();
        for result in &cmd.results {
            let winner_hex = hex::encode(&result.winner_root);
            *stack_changes.entry(winner_hex).or_insert(0) += result.amount;
        }

        let event = HandEnded {
            hand_root: cmd.hand_root,
            results: cmd.results,
            stack_changes,
            ended_at: Some(angzarr_client::now()),
        };

        Ok(new_event_book(cb, seq, &event, "examples.HandEnded"))
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn new_event_book<M: prost::Message>(
    cb: &CommandBook,
    seq: u32,
    event: &M,
    type_name: &str,
) -> EventBook {
    let event_any = Any {
        type_url: format!("type.googleapis.com/{}", type_name),
        value: event.encode_to_vec(),
    };

    EventBook {
        cover: cb.cover.clone(),
        pages: vec![EventPage {
            sequence: Some(event_page::Sequence::Num(seq)),
            event: Some(event_any),
            created_at: Some(angzarr_client::now()),
            external_payload: None,
        }],
        snapshot: None,
        next_sequence: 0,
    }
}

fn generate_hand_root(table_root: &[u8], hand_number: i64) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(table_root);
    hasher.update(hand_number.to_be_bytes());
    hasher.finalize().to_vec()
}

fn advance_to_next_active(current_pos: i32, state: &TableState) -> i32 {
    let max_players = state.max_players;
    for i in 1..=max_players {
        let next_pos = (current_pos + i) % max_players;
        if let Some(seat) = state.seats.get(&next_pos) {
            if !seat.is_sitting_out {
                return next_pos;
            }
        }
    }
    current_pos
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let agg = TableAggregate;
    let router = agg.into_router();

    println!("Starting Table aggregate (OO pattern)");
    println!("Domain: {}", router.domain());

    run_aggregate_server("table", 50012, router)
        .await
        .expect("Server failed");
}
