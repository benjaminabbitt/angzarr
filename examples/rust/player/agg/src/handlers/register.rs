//! RegisterPlayer command handler.

use angzarr_client::proto::examples::{PlayerRegistered, RegisterPlayer};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

fn guard(state: &PlayerState) -> CommandResult<()> {
    if state.exists() {
        return Err(CommandRejectedError::new("Player already exists"));
    }
    Ok(())
}

fn validate(cmd: &RegisterPlayer) -> CommandResult<()> {
    if cmd.display_name.is_empty() {
        return Err(CommandRejectedError::new("display_name is required"));
    }
    if cmd.email.is_empty() {
        return Err(CommandRejectedError::new("email is required"));
    }
    Ok(())
}

fn compute(cmd: &RegisterPlayer) -> PlayerRegistered {
    PlayerRegistered {
        display_name: cmd.display_name.clone(),
        email: cmd.email.clone(),
        player_type: cmd.player_type,
        ai_model_id: cmd.ai_model_id.clone(),
        registered_at: Some(angzarr_client::now()),
    }
}

pub fn handle_register_player(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: RegisterPlayer = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    validate(&cmd)?;

    let event = compute(&cmd);
    let event_any = pack_event(&event, "examples.PlayerRegistered");

    Ok(new_event_book(command_book, seq, event_any))
}
