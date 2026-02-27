//! SitOut command handler.

use angzarr_client::proto::examples::{PlayerSittingOut, SitOut};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

fn guard(state: &PlayerState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }
    Ok(())
}

fn validate(cmd: &SitOut, state: &PlayerState) -> CommandResult<()> {
    if cmd.table_root.is_empty() {
        return Err(CommandRejectedError::new("table_root is required"));
    }

    let table_key = hex::encode(&cmd.table_root);
    if !state.table_reservations.contains_key(&table_key) {
        return Err(CommandRejectedError::new("Player is not at this table"));
    }
    Ok(())
}

fn compute(cmd: &SitOut) -> PlayerSittingOut {
    PlayerSittingOut {
        table_root: cmd.table_root.clone(),
        sat_out_at: Some(angzarr_client::now()),
    }
}

pub fn handle_sit_out(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: SitOut = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    validate(&cmd, state)?;

    let event = compute(&cmd);
    let event_any = pack_event(&event, "examples.PlayerSittingOut");

    Ok(new_event_book(command_book, seq, event_any))
}
