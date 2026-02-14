//! CreateTable command handler.

use angzarr_client::proto::examples::{CreateTable, TableCreated};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::TableState;

fn guard(state: &TableState) -> CommandResult<()> {
    if state.exists() {
        return Err(CommandRejectedError::new("Table already exists"));
    }
    Ok(())
}

fn validate(cmd: &CreateTable) -> CommandResult<()> {
    if cmd.table_name.is_empty() {
        return Err(CommandRejectedError::new("table_name is required"));
    }
    if cmd.small_blind <= 0 {
        return Err(CommandRejectedError::new("small_blind must be positive"));
    }
    if cmd.big_blind <= 0 || cmd.big_blind < cmd.small_blind {
        return Err(CommandRejectedError::new("big_blind must be >= small_blind"));
    }
    if cmd.min_buy_in <= 0 {
        return Err(CommandRejectedError::new("min_buy_in must be positive"));
    }
    if cmd.max_buy_in < cmd.min_buy_in {
        return Err(CommandRejectedError::new("max_buy_in must be >= min_buy_in"));
    }
    if cmd.max_players < 2 || cmd.max_players > 10 {
        return Err(CommandRejectedError::new("max_players must be 2-10"));
    }
    Ok(())
}

fn compute(cmd: &CreateTable) -> TableCreated {
    TableCreated {
        table_name: cmd.table_name.clone(),
        game_variant: cmd.game_variant,
        small_blind: cmd.small_blind,
        big_blind: cmd.big_blind,
        min_buy_in: cmd.min_buy_in,
        max_buy_in: cmd.max_buy_in,
        max_players: cmd.max_players,
        action_timeout_seconds: cmd.action_timeout_seconds,
        created_at: Some(angzarr_client::now()),
    }
}

pub fn handle_create_table(
    command_book: &CommandBook,
    command_any: &Any,
    state: &TableState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: CreateTable = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    validate(&cmd)?;

    let event = compute(&cmd);
    let event_any = pack_event(&event, "examples.TableCreated");

    Ok(new_event_book(command_book, seq, event_any))
}
