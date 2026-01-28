//! Create cart command handler.

use prost::Message;

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CartCreated, CartState, CreateCart};
use common::{decode_command, make_event_book, now, require_not_exists, Result};

use crate::errmsg;

/// Handle the CreateCart command.
///
/// Creates a new cart for a customer. Fails if cart already exists.
pub fn handle_create_cart(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    require_not_exists(&state.customer_id, errmsg::CART_EXISTS)?;

    let cmd: CreateCart = decode_command(command_data)?;

    let event = CartCreated {
        customer_id: cmd.customer_id.clone(),
        created_at: Some(now()),
    };

    let new_state = CartState {
        customer_id: cmd.customer_id,
        items: vec![],
        subtotal_cents: 0,
        coupon_code: String::new(),
        discount_cents: 0,
        status: "active".to_string(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CartCreated",
        event.encode_to_vec(),
        "type.examples/examples.CartState",
        new_state.encode_to_vec(),
    ))
}
