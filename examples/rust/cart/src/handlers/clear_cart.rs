//! Clear cart command handler.

use prost::Message;

use common::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CartCleared, CartState};

use crate::errmsg;
use crate::state::now;

/// Handle the ClearCart command.
///
/// Removes all items from the cart and resets coupon.
pub fn handle_clear_cart(
    command_book: &CommandBook,
    _command_data: &[u8],
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

    let event = CartCleared {
        new_subtotal: 0,
        cleared_at: Some(now()),
    };

    let new_state = CartState {
        customer_id: state.customer_id.clone(),
        items: vec![],
        subtotal_cents: 0,
        coupon_code: String::new(),
        discount_cents: 0,
        status: state.status.clone(),
    };

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.CartCleared".to_string(),
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
