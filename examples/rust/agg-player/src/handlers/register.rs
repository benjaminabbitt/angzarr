//! RegisterPlayer command handler.

use angzarr_client::proto::examples::{PlayerRegistered, RegisterPlayer};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

pub fn handle_register_player(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    if state.exists() {
        return Err(CommandRejectedError::new("Player already exists"));
    }

    let cmd: RegisterPlayer = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    if cmd.display_name.is_empty() {
        return Err(CommandRejectedError::new("display_name is required"));
    }
    if cmd.email.is_empty() {
        return Err(CommandRejectedError::new("email is required"));
    }

    let event = PlayerRegistered {
        display_name: cmd.display_name,
        email: cmd.email,
        player_type: cmd.player_type,
        ai_model_id: cmd.ai_model_id,
        registered_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.PlayerRegistered");

    Ok(EventBook {
        cover: command_book.cover.clone(),
        pages: vec![EventPage {
            sequence: Some(event_page::Sequence::Num(seq)),
            event: Some(event_any),
            created_at: Some(angzarr_client::now()),
        }],
        snapshot: None,
        next_sequence: 0,
    })
}
