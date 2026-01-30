//! Handler for CreateCustomer command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CreateCustomer, CustomerCreated, CustomerState};
use common::{decode_command, now, require_exists, require_not_exists, Result};

use crate::errmsg;
use crate::state::build_event_response;

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
    require_not_exists(&state.name, errmsg::CUSTOMER_EXISTS)?;

    let cmd: CreateCustomer = decode_command(command_data)?;

    require_exists(&cmd.name, errmsg::NAME_REQUIRED)?;
    require_exists(&cmd.email, errmsg::EMAIL_REQUIRED)?;

    let event = CustomerCreated {
        name: cmd.name.clone(),
        email: cmd.email.clone(),
        created_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CustomerCreated",
        event,
    ))
}
