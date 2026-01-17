//! Handler for RedeemLoyaltyPoints command.

use angzarr::interfaces::business_client::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CustomerState, LoyaltyPointsRedeemed, RedeemLoyaltyPoints};
use prost::Message;

use super::{now, CUSTOMER_STATE_TYPE_URL, EVENT_TYPE_PREFIX};
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
    if state.name.is_empty() {
        return Err(BusinessError::Rejected(
            errmsg::CUSTOMER_NOT_FOUND.to_string(),
        ));
    }

    let cmd = RedeemLoyaltyPoints::decode(command_data)
        .map_err(|e| BusinessError::Rejected(e.to_string()))?;

    if cmd.points <= 0 {
        return Err(BusinessError::Rejected(errmsg::POINTS_POSITIVE.to_string()));
    }
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

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: format!("{}LoyaltyPointsRedeemed", EVENT_TYPE_PREFIX),
                value: event.encode_to_vec(),
            }),
            created_at: Some(now()),
            synchronous: false,
        }],
        correlation_id: String::new(),
        snapshot_state: Some(prost_types::Any {
            type_url: CUSTOMER_STATE_TYPE_URL.to_string(),
            value: new_state.encode_to_vec(),
        }),
    })
}
