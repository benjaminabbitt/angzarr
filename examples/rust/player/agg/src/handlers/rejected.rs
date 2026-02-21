//! Rejection handlers for saga/PM compensation.

use angzarr_client::proto::examples::{Currency, FundsReleased};
use angzarr_client::proto::{EventBook, Notification, RejectionNotification};
use angzarr_client::router::RejectionHandlerResponse;
use angzarr_client::{event_page, now, pack_event, CommandResult, UnpackAny};
use tracing::warn;

use crate::state::PlayerState;

// docs:start:rejected_handler

/// Handle JoinTable rejection by releasing reserved funds.
///
/// Called when the JoinTable command (issued by saga-player-table after
/// FundsReserved) is rejected by the Table aggregate.
pub fn handle_join_rejected(
    notification: &Notification,
    state: &PlayerState,
) -> CommandResult<RejectionHandlerResponse> {
    // Extract rejection details from the notification payload
    let rejection = notification
        .payload
        .as_ref()
        .and_then(|any| any.unpack::<RejectionNotification>().ok())
        .unwrap_or_default();

    warn!(
        rejection_reason = %rejection.rejection_reason,
        "Player compensation for JoinTable rejection"
    );

    // Extract table_root from the rejected command
    let table_root = rejection
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

    // Build the EventBook using the notification's cover for routing.
    // Sequence 0 is a placeholder - framework assigns actual sequence during persist.
    let event_book = EventBook {
        cover: notification.cover.clone(),
        pages: vec![event_page(0, event_any)],
        snapshot: None,
        next_sequence: 0,
    };

    Ok(RejectionHandlerResponse {
        events: Some(event_book),
        notification: None,
    })
}

// docs:end:rejected_handler
