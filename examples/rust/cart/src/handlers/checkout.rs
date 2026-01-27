//! Checkout command handler.

use prost::Message;

use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CartCheckedOut, CartState};
use common::{BusinessError, Result};

use crate::errmsg;
use crate::state::now;

/// Handle the Checkout command.
///
/// Finalizes the cart and marks it as checked out.
pub fn handle_checkout(
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
    if state.items.is_empty() {
        return Err(BusinessError::Rejected(errmsg::CART_EMPTY.to_string()));
    }

    let event = CartCheckedOut {
        final_subtotal: state.subtotal_cents,
        discount_cents: state.discount_cents,
        checked_out_at: Some(now()),
    };

    let new_state = CartState {
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
        subtotal_cents: state.subtotal_cents,
        coupon_code: state.coupon_code.clone(),
        discount_cents: state.discount_cents,
        status: "checked_out".to_string(),
    };

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.CartCheckedOut".to_string(),
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
