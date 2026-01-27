//! Update quantity command handler.

use prost::Message;

use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CartState, QuantityUpdated, UpdateQuantity};
use common::{BusinessError, Result};

use crate::errmsg;
use crate::state::{calculate_subtotal, now};

/// Handle the UpdateQuantity command.
///
/// Updates the quantity of an existing item in the cart.
pub fn handle_update_quantity(
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
        UpdateQuantity::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    if cmd.new_quantity <= 0 {
        return Err(BusinessError::Rejected(
            errmsg::QUANTITY_POSITIVE.to_string(),
        ));
    }

    let item = state
        .items
        .iter()
        .find(|i| i.product_id == cmd.product_id)
        .ok_or_else(|| BusinessError::Rejected(errmsg::ITEM_NOT_IN_CART.to_string()))?;

    let old_quantity = item.quantity;

    // Calculate new subtotal
    let mut items = state.items.clone();
    if let Some(i) = items.iter_mut().find(|i| i.product_id == cmd.product_id) {
        i.quantity = cmd.new_quantity;
    }
    let new_subtotal = calculate_subtotal(&items);

    let event = QuantityUpdated {
        product_id: cmd.product_id,
        old_quantity,
        new_quantity: cmd.new_quantity,
        new_subtotal,
        updated_at: Some(now()),
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
                type_url: "type.examples/examples.QuantityUpdated".to_string(),
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
