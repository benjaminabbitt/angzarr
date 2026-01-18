//! Apply coupon command handler.

use prost::Message;

use angzarr::clients::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{ApplyCoupon, CartState, CouponApplied};

use crate::errmsg;
use crate::state::now;

/// Handle the ApplyCoupon command.
///
/// Applies a coupon to the cart. Supports percentage and fixed discounts.
pub fn handle_apply_coupon(
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
    if !state.coupon_code.is_empty() {
        return Err(BusinessError::Rejected(
            errmsg::COUPON_ALREADY_APPLIED.to_string(),
        ));
    }
    if state.items.is_empty() {
        return Err(BusinessError::Rejected(errmsg::CART_EMPTY.to_string()));
    }

    let cmd =
        ApplyCoupon::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    let discount_cents = if cmd.coupon_type == "percentage" {
        (state.subtotal_cents * cmd.value) / 100
    } else {
        // fixed
        cmd.value
    };

    let event = CouponApplied {
        coupon_code: cmd.code.clone(),
        coupon_type: cmd.coupon_type.clone(),
        value: cmd.value,
        discount_cents,
        applied_at: Some(now()),
    };

    let new_state = CartState {
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
        subtotal_cents: state.subtotal_cents,
        coupon_code: cmd.code,
        discount_cents,
        status: state.status.clone(),
    };

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.CouponApplied".to_string(),
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
