//! LeaveTable command handler.

use angzarr_client::proto::examples::{LeaveTable, PlayerLeft};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::{SeatState, TableState};

fn guard(state: &TableState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Table does not exist"));
    }
    if state.status == "in_hand" {
        return Err(CommandRejectedError::new("Cannot leave during a hand"));
    }
    Ok(())
}

fn validate<'a>(cmd: &LeaveTable, state: &'a TableState) -> CommandResult<(i32, &'a SeatState)> {
    if cmd.player_root.is_empty() {
        return Err(CommandRejectedError::new("player_root is required"));
    }

    let seat_position = state
        .find_seat_by_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not seated at table"))?;

    let seat = state.seats.get(&seat_position).unwrap();

    Ok((seat_position, seat))
}

fn compute(cmd: &LeaveTable, seat_position: i32, seat: &SeatState) -> PlayerLeft {
    PlayerLeft {
        player_root: cmd.player_root.clone(),
        seat_position,
        chips_cashed_out: seat.stack,
        left_at: Some(angzarr_client::now()),
    }
}

pub fn handle_leave_table(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: LeaveTable = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    let (seat_position, seat) = validate(&cmd, state)?;

    let event = compute(&cmd, seat_position, seat);
    let event_any = pack_event(&event, "examples.PlayerLeft");

    Ok(new_event_book(command_book, seq, event_any))
}
