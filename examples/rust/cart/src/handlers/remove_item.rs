//! Remove item command handler.

use prost::Message;

use common::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CartItem, CartState, ItemRemoved, RemoveItem};

use crate::errmsg;
use crate::state::{calculate_subtotal, now};

/// Handle the RemoveItem command.
///
/// Removes an item from the cart entirely.
pub fn handle_remove_item(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    if state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
    }
    if state.status == "checked_out" {
        return Err(BusinessError::Rejected(
            errmsg::CART_CHECKED_OUT.to_string(),
        ));
    }

    let cmd =
        RemoveItem::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    let item = state
        .items
        .iter()
        .find(|i| i.product_id == cmd.product_id)
        .ok_or_else(|| BusinessError::Rejected(errmsg::ITEM_NOT_IN_CART.to_string()))?;

    let removed_quantity = item.quantity;

    // Calculate new subtotal
    let items: Vec<CartItem> = state
        .items
        .iter()
        .filter(|i| i.product_id != cmd.product_id)
        .cloned()
        .collect();
    let new_subtotal = calculate_subtotal(&items);

    let event = ItemRemoved {
        product_id: cmd.product_id,
        quantity: removed_quantity,
        new_subtotal,
        removed_at: Some(now()),
    };

    let new_state = CartState {
        customer_id: state.customer_id.clone(),
        items,
        subtotal_cents: new_subtotal,
        coupon_code: state.coupon_code.clone(),
        discount_cents: state.discount_cents,
        status: state.status.clone(),
    };

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.ItemRemoved".to_string(),
                value: event.encode_to_vec(),
            }),
            created_at: Some(now()),
        }],
        correlation_id: String::new(),
        snapshot_state: Some(prost_types::Any {
            type_url: "type.examples/examples.CartState".to_string(),
            value: new_state.encode_to_vec(),
        }),
    })
}
