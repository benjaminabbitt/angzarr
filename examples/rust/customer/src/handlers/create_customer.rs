//! Handler for CreateCustomer command.

use angzarr::interfaces::business_client::{BusinessError, Result};
use angzarr::proto::{event_page::Sequence, CommandBook, EventBook, EventPage};
use common::proto::{CreateCustomer, CustomerCreated, CustomerState};
use prost::Message;

use super::{now, CUSTOMER_STATE_TYPE_URL, EVENT_TYPE_PREFIX};
use crate::errmsg;

/// Handle the CreateCustomer command.
///
/// Creates a new customer with the given name and email.
/// Fails if the customer already exists (non-empty name in state).
pub fn handle_create_customer(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CustomerState,
    next_seq: u32,
) -> Result<EventBook> {
    if !state.name.is_empty() {
        return Err(BusinessError::Rejected(errmsg::CUSTOMER_EXISTS.to_string()));
    }

    let cmd =
        CreateCustomer::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    if cmd.name.is_empty() {
        return Err(BusinessError::Rejected(errmsg::NAME_REQUIRED.to_string()));
    }
    if cmd.email.is_empty() {
        return Err(BusinessError::Rejected(errmsg::EMAIL_REQUIRED.to_string()));
    }

    let event = CustomerCreated {
        name: cmd.name.clone(),
        email: cmd.email.clone(),
        created_at: Some(now()),
    };

    // New state after applying event
    let new_state = CustomerState {
        name: cmd.name,
        email: cmd.email,
        loyalty_points: 0,
        lifetime_points: 0,
    };

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            event: Some(prost_types::Any {
                type_url: format!("{}CustomerCreated", EVENT_TYPE_PREFIX),
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
