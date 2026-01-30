//! Handler for AddLoyaltyPoints command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{AddLoyaltyPoints, CustomerState, LoyaltyPointsAdded};
use common::{decode_command, require_exists, require_positive, Result};

use crate::errmsg;
use crate::state::build_event_response;

/// Handle the AddLoyaltyPoints command.
///
/// Adds loyalty points to an existing customer's balance.
/// Fails if the customer does not exist or points are not positive.
pub fn handle_add_loyalty_points(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CustomerState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.name, errmsg::CUSTOMER_NOT_FOUND)?;

    let cmd: AddLoyaltyPoints = decode_command(command_data)?;

    require_positive(cmd.points, errmsg::POINTS_POSITIVE)?;

    let new_balance = state.loyalty_points + cmd.points;
    let new_lifetime_points = state.lifetime_points + cmd.points;

    let event = LoyaltyPointsAdded {
        points: cmd.points,
        new_balance,
        reason: cmd.reason,
        new_lifetime_points, // Fact: total lifetime points after this event
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.LoyaltyPointsAdded",
        event,
    ))
}
