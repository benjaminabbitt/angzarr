//! Handler for RedeemLoyaltyPoints command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CustomerState, LoyaltyPointsRedeemed, RedeemLoyaltyPoints};
use common::{
    decode_command, make_event_book, require_exists, require_positive, BusinessError, Result,
};
use prost::Message;

use crate::errmsg;

/// Handle the RedeemLoyaltyPoints command.
///
/// Redeems loyalty points from an existing customer's balance.
/// Fails if the customer does not exist, points are not positive,
/// or the customer has insufficient points.
pub fn handle_redeem_loyalty_points(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CustomerState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.name, errmsg::CUSTOMER_NOT_FOUND)?;

    let cmd: RedeemLoyaltyPoints = decode_command(command_data)?;

    require_positive(cmd.points, errmsg::POINTS_POSITIVE)?;
    if cmd.points > state.loyalty_points {
        return Err(BusinessError::Rejected(format!(
            "{}: have {}, need {}",
            errmsg::INSUFFICIENT_POINTS,
            state.loyalty_points,
            cmd.points
        )));
    }

    let new_balance = state.loyalty_points - cmd.points;

    let event = LoyaltyPointsRedeemed {
        points: cmd.points,
        new_balance,
        redemption_type: cmd.redemption_type,
    };

    // New state after applying event (lifetime_points unchanged on redemption)
    let new_state = CustomerState {
        name: state.name.clone(),
        email: state.email.clone(),
        loyalty_points: new_balance,
        lifetime_points: state.lifetime_points,
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.LoyaltyPointsRedeemed",
        event.encode_to_vec(),
        "type.examples/examples.CustomerState",
        new_state.encode_to_vec(),
    ))
}
