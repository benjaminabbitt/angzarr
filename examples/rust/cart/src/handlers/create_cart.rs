//! Create cart command handler.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CartCreated, CartState, CreateCart};
use common::{decode_command, now, require_not_exists, Result};

use crate::errmsg;
use crate::state::build_event_response;

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

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CartCreated",
        event,
    ))
}
