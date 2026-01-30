//! Handler for RedeemLoyaltyPoints command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CustomerState, LoyaltyPointsRedeemed, RedeemLoyaltyPoints};
use common::{decode_command, require_exists, require_positive, BusinessError, Result};

use crate::errmsg;
use crate::state::build_event_response;

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

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.LoyaltyPointsRedeemed",
        event,
    ))
}
