//! Add item command handler.

use prost::Message;

use common::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{AddItem, CartItem, CartState, ItemAdded};

use crate::errmsg;
use crate::state::{calculate_subtotal, now};

/// Handle the AddItem command.
///
/// Adds an item to the cart or increases quantity if already present.
pub fn handle_add_item(
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

    let cmd = AddItem::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    if cmd.quantity <= 0 {
        return Err(BusinessError::Rejected(
            errmsg::QUANTITY_POSITIVE.to_string(),
        ));
    }

    // Calculate new quantity (add to existing if present)
    let existing_qty = state
        .items
        .iter()
        .find(|i| i.product_id == cmd.product_id)
        .map(|i| i.quantity)
        .unwrap_or(0);
    let new_quantity = existing_qty + cmd.quantity;

    // Calculate new subtotal
    let mut items = state.items.clone();
    if let Some(item) = items.iter_mut().find(|i| i.product_id == cmd.product_id) {
        item.quantity = new_quantity;
    } else {
        items.push(CartItem {
            product_id: cmd.product_id.clone(),
            name: cmd.name.clone(),
            quantity: cmd.quantity,
            unit_price_cents: cmd.unit_price_cents,
        });
    }
    let new_subtotal = calculate_subtotal(&items);

    let event = ItemAdded {
        product_id: cmd.product_id.clone(),
        name: cmd.name.clone(),
        quantity: new_quantity,
        unit_price_cents: cmd.unit_price_cents,
        new_subtotal,
        added_at: Some(now()),
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
                type_url: "type.examples/examples.ItemAdded".to_string(),
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
