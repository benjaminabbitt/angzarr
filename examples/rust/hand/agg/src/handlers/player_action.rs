//! PlayerAction command handler.

use angzarr_client::proto::examples::{ActionTaken, ActionType, PlayerAction};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::HandState;

pub fn handle_player_action(
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
    if state.status != "betting" {
        return Err(CommandRejectedError::new("Not in betting phase"));
    }

    let cmd: PlayerAction = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    let player = state
        .get_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not in hand"))?;

    if player.has_folded {
        return Err(CommandRejectedError::new("Player has folded"));
    }
    if player.is_all_in {
        return Err(CommandRejectedError::new("Player is all-in"));
    }

    // Validate action
    let action = ActionType::try_from(cmd.action).unwrap_or_default();
    let amount_to_call = state.current_bet - player.bet_this_round;
    let chips_put_in: i64; // Actual chips added to pot
    let event_amount: i64; // Amount to record in event

    match action {
        ActionType::Fold => {
            chips_put_in = 0;
            event_amount = 0;
        }
        ActionType::Check => {
            if amount_to_call > 0 {
                return Err(CommandRejectedError::new("Cannot check, must call or fold"));
            }
            chips_put_in = 0;
            event_amount = 0;
        }
        ActionType::Call => {
            if amount_to_call <= 0 {
                return Err(CommandRejectedError::new("Nothing to call"));
            }
            chips_put_in = amount_to_call.min(player.stack);
            event_amount = chips_put_in;
        }
        ActionType::Bet => {
            if state.current_bet > 0 {
                return Err(CommandRejectedError::new("Cannot bet, use raise"));
            }
            if cmd.amount < state.min_raise {
                return Err(CommandRejectedError::new(format!(
                    "Bet must be at least {}",
                    state.min_raise
                )));
            }
            chips_put_in = cmd.amount.min(player.stack);
            event_amount = chips_put_in;
        }
        ActionType::Raise => {
            if state.current_bet <= 0 {
                return Err(CommandRejectedError::new("Cannot raise, use bet"));
            }
            let raise_amount = cmd.amount - state.current_bet;
            if raise_amount < state.min_raise {
                return Err(CommandRejectedError::new("Raise below minimum"));
            }
            let to_put_in = cmd.amount - player.bet_this_round;
            chips_put_in = to_put_in.min(player.stack);
            // Record total bet (cmd.amount) in event for raises
            event_amount = cmd.amount;
        }
        ActionType::AllIn => {
            chips_put_in = player.stack;
            event_amount = chips_put_in;
        }
        _ => {
            return Err(CommandRejectedError::new("Unknown action"));
        }
    }

    let new_stack = player.stack - chips_put_in;
    let new_pot = state.total_pot() + chips_put_in;

    // Calculate new current bet
    let player_total_bet = player.bet_this_round + chips_put_in;
    let new_current_bet = state.current_bet.max(player_total_bet);

    // Determine final action type
    let final_action = if chips_put_in == player.stack && chips_put_in > 0 {
        ActionType::AllIn
    } else {
        action
    };

    let event = ActionTaken {
        player_root: cmd.player_root,
        action: final_action as i32,
        amount: event_amount,
        player_stack: new_stack,
        pot_total: new_pot,
        amount_to_call: new_current_bet,
        action_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.ActionTaken");

    Ok(new_event_book(command_book, seq, event_any))
}
