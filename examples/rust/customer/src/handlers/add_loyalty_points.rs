//! Handler for AddLoyaltyPoints command.

use angzarr::clients::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{AddLoyaltyPoints, CustomerState, LoyaltyPointsAdded};
use prost::Message;

use super::{now, CUSTOMER_STATE_TYPE_URL, EVENT_TYPE_PREFIX};
use crate::errmsg;

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
    if state.name.is_empty() {
        return Err(BusinessError::Rejected(
            errmsg::CUSTOMER_NOT_FOUND.to_string(),
        ));
    }

    let cmd = AddLoyaltyPoints::decode(command_data)
        .map_err(|e| BusinessError::Rejected(e.to_string()))?;

    if cmd.points <= 0 {
        return Err(BusinessError::Rejected(errmsg::POINTS_POSITIVE.to_string()));
    }

    let new_balance = state.loyalty_points + cmd.points;
    let new_lifetime_points = state.lifetime_points + cmd.points;

    let event = LoyaltyPointsAdded {
        points: cmd.points,
        new_balance,
        reason: cmd.reason,
        new_lifetime_points, // Fact: total lifetime points after this event
    };

    // New state after applying event
    let new_state = CustomerState {
        name: state.name.clone(),
        email: state.email.clone(),
        loyalty_points: new_balance,
        lifetime_points: new_lifetime_points,
    };

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: format!("{}LoyaltyPointsAdded", EVENT_TYPE_PREFIX),
                value: event.encode_to_vec(),
            }),
            created_at: Some(now()),
        }],
        correlation_id: String::new(),
        snapshot_state: Some(prost_types::Any {
            type_url: CUSTOMER_STATE_TYPE_URL.to_string(),
            value: new_state.encode_to_vec(),
        }),
    })
}
