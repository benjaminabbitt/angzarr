//! StartHand command handler.

use sha2::{Sha256, Digest};

use angzarr_client::proto::examples::{HandStarted, SeatSnapshot, StartHand};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::TableState;

fn guard(state: &TableState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Table does not exist"));
    }
    if state.status == "in_hand" {
        return Err(CommandRejectedError::new("Hand already in progress"));
    }
    if state.active_player_count() < 2 {
        return Err(CommandRejectedError::new("Not enough players to start hand"));
    }
    Ok(())
}

fn compute(state: &TableState, table_root: &[u8]) -> HandStarted {
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

    HandStarted {
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
    }
}

pub fn handle_start_hand(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    let _cmd: StartHand = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;

    let table_root = command_book
        .cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .map(|u| u.value.as_slice())
        .unwrap_or(&[]);

    let event = compute(state, table_root);
    let event_any = pack_event(&event, "examples.HandStarted");

    Ok(new_event_book(command_book, seq, event_any))
}

/// Generate deterministic hand root from table root and hand number.
fn generate_hand_root(table_root: &[u8], hand_number: i64) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(table_root);
    hasher.update(hand_number.to_be_bytes());
    hasher.finalize().to_vec()
}

/// Find the next active (non-sitting-out) player position.
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
    current_pos // Shouldn't happen if we have active players
}
