//! PostBlind command handler.

use angzarr_client::proto::examples::{BlindPosted, PostBlind};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::HandState;

pub fn handle_post_blind(
    command_book: &CommandBook,
    command_any: &Any,
    state: &HandState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Hand does not exist"));
    }
    if state.is_complete() {
        return Err(CommandRejectedError::new("Hand already complete"));
    }

    let cmd: PostBlind = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    let player = state
        .get_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not in hand"))?;

    if cmd.amount <= 0 {
        return Err(CommandRejectedError::new("Amount must be positive"));
    }

    // Calculate actual amount (might be all-in)
    let actual_amount = cmd.amount.min(player.stack);
    let new_stack = player.stack - actual_amount;
    let new_pot = state.total_pot() + actual_amount;

    let event = BlindPosted {
        player_root: cmd.player_root,
        blind_type: cmd.blind_type,
        amount: actual_amount,
        player_stack: new_stack,
        pot_total: new_pot,
        posted_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.BlindPosted");

    Ok(new_event_book(command_book, seq, event_any))
}
