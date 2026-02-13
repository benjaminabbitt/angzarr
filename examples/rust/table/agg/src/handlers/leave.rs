//! LeaveTable command handler.

use angzarr_client::proto::examples::{LeaveTable, PlayerLeft};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::TableState;

pub fn handle_leave_table(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Table does not exist"));
    }

    let cmd: LeaveTable = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    if cmd.player_root.is_empty() {
        return Err(CommandRejectedError::new("player_root is required"));
    }

    let seat_position = state
        .find_seat_by_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not seated at table"))?;

    // Can't leave during a hand
    if state.status == "in_hand" {
        return Err(CommandRejectedError::new("Cannot leave during a hand"));
    }

    let seat = state.seats.get(&seat_position).unwrap();

    let event = PlayerLeft {
        player_root: cmd.player_root,
        seat_position,
        chips_cashed_out: seat.stack,
        left_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.PlayerLeft");

    Ok(new_event_book(command_book, seq, event_any))
}
