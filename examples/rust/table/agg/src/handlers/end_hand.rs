//! EndHand command handler.

use std::collections::HashMap;

use angzarr_client::proto::examples::{EndHand, HandEnded};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::TableState;

fn guard(state: &TableState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Table does not exist"));
    }
    if state.status != "in_hand" {
        return Err(CommandRejectedError::new("No hand in progress"));
    }
    Ok(())
}

fn validate(cmd: &EndHand, state: &TableState) -> CommandResult<()> {
    if hex::encode(&cmd.hand_root) != hex::encode(&state.current_hand_root) {
        return Err(CommandRejectedError::new("Hand root mismatch"));
    }
    Ok(())
}

fn compute(cmd: &EndHand) -> HandEnded {
    let mut stack_changes: HashMap<String, i64> = HashMap::new();
    for result in &cmd.results {
        let winner_hex = hex::encode(&result.winner_root);
        *stack_changes.entry(winner_hex).or_insert(0) += result.amount;
    }

    HandEnded {
        hand_root: cmd.hand_root.clone(),
        results: cmd.results.clone(),
        stack_changes,
        ended_at: Some(angzarr_client::now()),
    }
}

pub fn handle_end_hand(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: EndHand = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    validate(&cmd, state)?;

    let event = compute(&cmd);
    let event_any = pack_event(&event, "examples.HandEnded");

    Ok(new_event_book(command_book, seq, event_any))
}
