//! EndHand command handler.

use std::collections::HashMap;

use angzarr_client::proto::examples::{EndHand, HandEnded};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::TableState;

pub fn handle_end_hand(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Table does not exist"));
    }

    let cmd: EndHand = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    // Verify we're in a hand
    if state.status != "in_hand" {
        return Err(CommandRejectedError::new("No hand in progress"));
    }

    // Verify hand root matches
    if hex::encode(&cmd.hand_root) != hex::encode(&state.current_hand_root) {
        return Err(CommandRejectedError::new("Hand root mismatch"));
    }

    // Calculate stack changes from pot results
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

    let event_any = pack_event(&event, "examples.HandEnded");

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
