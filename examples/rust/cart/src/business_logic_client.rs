//! AggregateLogic implementation for CartLogic.

use angzarr::proto::{business_response, BusinessResponse, ContextualCommand};
use common::{next_sequence, AggregateLogic, BusinessError};

use crate::handlers::{
    handle_add_item, handle_apply_coupon, handle_checkout, handle_clear_cart, handle_create_cart,
    handle_remove_item, handle_update_quantity,
};
use crate::state::rebuild_state;
use crate::{errmsg, CartLogic};

#[tonic::async_trait]
impl AggregateLogic for CartLogic {
    async fn handle(&self, cmd: ContextualCommand) -> std::result::Result<BusinessResponse, tonic::Status> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = rebuild_state(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(
                errmsg::NO_COMMAND_PAGES.to_string(),
            ).into());
        };

        let command_page = cb
            .pages
            .first()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let command_any = command_page
            .command
            .as_ref()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let events = if command_any.type_url.ends_with("CreateCart") {
            handle_create_cart(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("AddItem") {
            handle_add_item(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("UpdateQuantity") {
            handle_update_quantity(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("RemoveItem") {
            handle_remove_item(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ApplyCoupon") {
            handle_apply_coupon(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ClearCart") {
            handle_clear_cart(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("Checkout") {
            handle_checkout(cb, &command_any.value, &state, next_seq)?
        } else {
            return Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            )).into());
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
    }
}
