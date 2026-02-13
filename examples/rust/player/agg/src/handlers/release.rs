//! ReleaseFunds command handler.

use angzarr_client::proto::examples::{Currency, FundsReleased, ReleaseFunds};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

pub fn handle_release_funds(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }

    let cmd: ReleaseFunds = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    if cmd.table_root.is_empty() {
        return Err(CommandRejectedError::new("table_root is required"));
    }

    let table_key = hex::encode(&cmd.table_root);
    let reserved = match state.table_reservations.get(&table_key) {
        Some(&amount) => amount,
        None => return Err(CommandRejectedError::new("No funds reserved for this table")),
    };

    let new_reserved = state.reserved_funds - reserved;
    let new_available = state.bankroll - new_reserved;

    let event = FundsReleased {
        amount: Some(Currency {
            amount: reserved,
            currency_code: "CHIPS".to_string(),
        }),
        table_root: cmd.table_root,
        new_available_balance: Some(Currency {
            amount: new_available,
            currency_code: "CHIPS".to_string(),
        }),
        new_reserved_balance: Some(Currency {
            amount: new_reserved,
            currency_code: "CHIPS".to_string(),
        }),
        released_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.FundsReleased");

    Ok(new_event_book(command_book, seq, event_any))
}
