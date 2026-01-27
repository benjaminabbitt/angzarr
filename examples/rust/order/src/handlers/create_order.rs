//! Handler for CreateOrder command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CreateOrder, OrderCreated, OrderState};
use common::{BusinessError, Result};
use prost::Message;

use super::{make_event_book, now};
use crate::errmsg;

/// Handle the CreateOrder command.
pub fn handle_create_order(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    if !state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::ORDER_EXISTS.to_string()));
    }

    let cmd =
        CreateOrder::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    if cmd.items.is_empty() {
        return Err(BusinessError::Rejected(errmsg::ITEMS_REQUIRED.to_string()));
    }

    for item in &cmd.items {
        if item.quantity <= 0 {
            return Err(BusinessError::Rejected(
                errmsg::QUANTITY_POSITIVE.to_string(),
            ));
        }
    }

    let subtotal: i32 = cmd
        .items
        .iter()
        .map(|i| i.quantity * i.unit_price_cents)
        .sum();

    let event = OrderCreated {
        customer_id: cmd.customer_id.clone(),
        items: cmd.items.clone(),
        subtotal_cents: subtotal,
        created_at: Some(now()),
    };

    let new_state = OrderState {
        customer_id: cmd.customer_id,
        items: cmd.items,
        subtotal_cents: subtotal,
        discount_cents: 0,
        loyalty_points_used: 0,
        payment_method: String::new(),
        payment_reference: String::new(),
        status: "pending".to_string(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCreated",
        event.encode_to_vec(),
        new_state.encode_to_vec(),
    ))
}
