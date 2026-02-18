//! Rejection handlers for saga/PM compensation.

use angzarr_client::proto::examples::{Currency, FundsReleased};
use angzarr_client::proto::{EventBook, Notification};
use angzarr_client::{new_event_book_from_notification, now, pack_event, CompensationContext};
use tracing::warn;

use crate::state::PlayerState;

// docs:start:rejected_handler

/// Handle JoinTable rejection by releasing reserved funds.
///
/// Called when the JoinTable command (issued by saga-player-table after
/// FundsReserved) is rejected by the Table aggregate.
pub fn handle_join_rejected(notification: &Notification, state: &PlayerState) -> EventBook {
    let ctx = CompensationContext::from_notification(notification);

    warn!(
        rejection_reason = %ctx.rejection_reason,
        "Player compensation for JoinTable rejection"
    );

    // Extract table_root from the rejected command
    let table_root = ctx
        .rejected_command
        .as_ref()
        .and_then(|cmd| cmd.cover.as_ref())
        .map(|cover| cover.root.as_ref().map(|r| r.value.clone()).unwrap_or_default())
        .unwrap_or_default();

    // Release the funds that were reserved for this table
    let table_key = hex::encode(&table_root);
    let reserved_amount = state.table_reservations.get(&table_key).copied().unwrap_or(0);
    let new_reserved = state.reserved_funds - reserved_amount;
    let new_available = state.bankroll - new_reserved;

    let event = FundsReleased {
        amount: Some(Currency {
            amount: reserved_amount,
            currency_code: "CHIPS".to_string(),
        }),
        table_root,
        reason: format!("Join failed: {}", ctx.rejection_reason),
        new_available_balance: Some(Currency {
            amount: new_available,
            currency_code: "CHIPS".to_string(),
        }),
        new_reserved_balance: Some(Currency {
            amount: new_reserved,
            currency_code: "CHIPS".to_string(),
        }),
        released_at: Some(now()),
    };

    let event_any = pack_event(&event, "examples.FundsReleased");
    new_event_book_from_notification(notification, event_any)
}

// docs:end:rejected_handler
