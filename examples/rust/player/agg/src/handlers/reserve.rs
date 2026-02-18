//! ReserveFunds command handler.
//!
//! DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//!      Update documentation when making changes to handler patterns.

use angzarr_client::proto::examples::{Currency, FundsReserved, ReserveFunds};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

// docs:start:reserve_funds_imp
fn guard(state: &PlayerState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }
    Ok(())
}

fn validate(cmd: &ReserveFunds, state: &PlayerState) -> CommandResult<i64> {
    let amount = cmd.amount.as_ref().map(|c| c.amount).unwrap_or(0);
    if amount <= 0 {
        return Err(CommandRejectedError::new("amount must be positive"));
    }
    if amount > state.available_balance() {
        return Err(CommandRejectedError::new("Insufficient funds"));
    }

    let table_key = hex::encode(&cmd.table_root);
    if state.table_reservations.contains_key(&table_key) {
        return Err(CommandRejectedError::new("Funds already reserved for this table"));
    }

    Ok(amount)
}

fn compute(cmd: &ReserveFunds, state: &PlayerState, amount: i64) -> FundsReserved {
    let new_reserved = state.reserved_funds + amount;
    let new_available = state.bankroll - new_reserved;

    FundsReserved {
        amount: cmd.amount.clone(),
        table_root: cmd.table_root.clone(),
        new_available_balance: Some(Currency {
            amount: new_available,
            currency_code: "CHIPS".to_string(),
        }),
        new_reserved_balance: Some(Currency {
            amount: new_reserved,
            currency_code: "CHIPS".to_string(),
        }),
        reserved_at: Some(angzarr_client::now()),
    }
}

pub fn handle_reserve_funds(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: ReserveFunds = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    let amount = validate(&cmd, state)?;

    let event = compute(&cmd, state, amount);
    let event_any = pack_event(&event, "examples.FundsReserved");

    Ok(new_event_book(command_book, seq, event_any))
}
// docs:end:reserve_funds_imp
