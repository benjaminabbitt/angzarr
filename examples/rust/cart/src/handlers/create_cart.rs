//! Create cart command handler.

use prost::Message;

use angzarr::interfaces::business_client::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CartCreated, CartState, CreateCart};

use crate::errmsg;
use crate::state::now;

/// Handle the CreateCart command.
///
/// Creates a new cart for a customer. Fails if cart already exists.
pub fn handle_create_cart(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    if !state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::CART_EXISTS.to_string()));
    }

    let cmd =
        CreateCart::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

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

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.CartCreated".to_string(),
                value: event.encode_to_vec(),
            }),
            created_at: Some(now()),
            synchronous: false,
        }],
        correlation_id: String::new(),
        snapshot_state: Some(prost_types::Any {
            type_url: "type.examples/examples.CartState".to_string(),
            value: new_state.encode_to_vec(),
        }),
    })
}
