//! ReserveFunds command handler.

use angzarr_client::proto::examples::{Currency, FundsReserved, ReserveFunds};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

pub fn handle_reserve_funds(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }

    let cmd: ReserveFunds = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    let amount = cmd.amount.as_ref().map(|c| c.amount).unwrap_or(0);
    if amount <= 0 {
        return Err(CommandRejectedError::new("amount must be positive"));
    }

    if amount > state.available_balance() {
        return Err(CommandRejectedError::new("Insufficient funds"));
    }

    // Check for existing reservation at this table
    let table_key = hex::encode(&cmd.table_root);
    if state.table_reservations.contains_key(&table_key) {
        return Err(CommandRejectedError::new("Funds already reserved for this table"));
    }

    let new_reserved = state.reserved_funds + amount;
    let new_available = state.bankroll - new_reserved;

    let event = FundsReserved {
        amount: cmd.amount.clone(),
        table_root: cmd.table_root,
        new_available_balance: Some(Currency {
            amount: new_available,
            currency_code: "CHIPS".to_string(),
        }),
        new_reserved_balance: Some(Currency {
            amount: new_reserved,
            currency_code: "CHIPS".to_string(),
        }),
        reserved_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.FundsReserved");

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
