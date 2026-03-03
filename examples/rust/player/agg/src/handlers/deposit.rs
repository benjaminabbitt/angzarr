//! DepositFunds command handler.

use angzarr_client::proto::examples::{Currency, DepositFunds, FundsDeposited};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::PlayerState;

// docs:start:deposit_guard
fn guard(state: &PlayerState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }
    Ok(())
}
// docs:end:deposit_guard

// docs:start:deposit_validate
fn validate(cmd: &DepositFunds) -> CommandResult<i64> {
    let amount = cmd.amount.as_ref().map(|c| c.amount).unwrap_or(0);
    if amount <= 0 {
        return Err(CommandRejectedError::new("amount must be positive"));
    }
    Ok(amount)
}
// docs:end:deposit_validate

// docs:start:deposit_compute
fn compute(cmd: &DepositFunds, state: &PlayerState, amount: i64) -> FundsDeposited {
    let new_balance = state.bankroll + amount;
    FundsDeposited {
        amount: cmd.amount.clone(),
        new_balance: Some(Currency {
            amount: new_balance,
            currency_code: "CHIPS".to_string(),
        }),
        deposited_at: Some(angzarr_client::now()),
    }
}
// docs:end:deposit_compute

// docs:start:polyglot_handler
pub fn handle_deposit_funds(
    command_book: &CommandBook,
    command_any: &Any,
    state: &PlayerState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: DepositFunds = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    let amount = validate(&cmd)?;

    let event = compute(&cmd, state, amount);
    let event_any = pack_event(&event, "examples.FundsDeposited");

    Ok(new_event_book(command_book, seq, event_any))
}
// docs:end:polyglot_handler

// docs:start:unit_test_deposit
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_increases_bankroll() {
        let state = PlayerState {
            player_id: "player_1".to_string(),
            bankroll: 1000,
            ..Default::default()
        };
        let cmd = DepositFunds {
            amount: Some(Currency {
                amount: 500,
                currency_code: "CHIPS".to_string(),
            }),
        };

        let event = compute(&cmd, &state, 500);

        assert_eq!(event.new_balance.unwrap().amount, 1500);
    }

    #[test]
    fn test_deposit_rejects_non_existent_player() {
        let state = PlayerState::default(); // player_id empty = doesn't exist

        let result = guard(&state);

        assert!(result.is_err());
        assert!(result.unwrap_err().reason.contains("does not exist"));
    }

    #[test]
    fn test_deposit_rejects_zero_amount() {
        let cmd = DepositFunds {
            amount: Some(Currency {
                amount: 0,
                currency_code: "CHIPS".to_string(),
            }),
        };

        let result = validate(&cmd);

        assert!(result.is_err());
        assert!(result.unwrap_err().reason.contains("positive"));
    }
}
// docs:end:unit_test_deposit
