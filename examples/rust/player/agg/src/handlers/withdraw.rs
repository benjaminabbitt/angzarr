//! WithdrawFunds command handler.

use angzarr_client::proto::examples::{Currency, FundsWithdrawn, WithdrawFunds};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

pub fn handle_withdraw_funds(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }

    let cmd: WithdrawFunds = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    let amount = cmd.amount.as_ref().map(|c| c.amount).unwrap_or(0);
    if amount <= 0 {
        return Err(CommandRejectedError::new("amount must be positive"));
    }

    if amount > state.available_balance() {
        return Err(CommandRejectedError::new("insufficient available balance"));
    }

    let new_balance = state.bankroll - amount;

    let event = FundsWithdrawn {
        amount: cmd.amount,
        new_balance: Some(Currency {
            amount: new_balance,
            currency_code: "CHIPS".to_string(),
        }),
        withdrawn_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.FundsWithdrawn");

    Ok(new_event_book(command_book, seq, event_any))
}
