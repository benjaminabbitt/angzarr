//! JoinTable command handler.

use angzarr_client::proto::examples::{JoinTable, PlayerJoined};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::TableState;

pub fn handle_join_table(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Table does not exist"));
    }

    let cmd: JoinTable = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    if cmd.player_root.is_empty() {
        return Err(CommandRejectedError::new("player_root is required"));
    }

    // Check if player already seated
    if state.find_seat_by_player(&cmd.player_root).is_some() {
        return Err(CommandRejectedError::new("Player already seated"));
    }

    // Validate buy-in
    if cmd.buy_in_amount < state.min_buy_in {
        return Err(CommandRejectedError::new(format!(
            "Buy-in must be at least {}",
            state.min_buy_in
        )));
    }
    if cmd.buy_in_amount > state.max_buy_in {
        return Err(CommandRejectedError::new("Buy-in above maximum"));
    }

    // Find seat
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

    let event = PlayerJoined {
        player_root: cmd.player_root,
        seat_position,
        buy_in_amount: cmd.buy_in_amount,
        stack: cmd.buy_in_amount,
        joined_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.PlayerJoined");

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
