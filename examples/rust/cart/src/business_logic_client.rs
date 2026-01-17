use async_trait::async_trait;
use angzarr::{BusinessLogicClient, ContextualCommand};
use angzarr::interfaces::{business_client, BusinessError};
use angzarr::proto::{business_response, BusinessResponse};
use common::next_sequence;
use crate::{errmsg, CartLogic};

#[async_trait]
impl BusinessLogicClient for CartLogic {
    async fn handle(&self, _domain: &str, cmd: ContextualCommand) -> business_client::Result<BusinessResponse> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(
                errmsg::NO_COMMAND_PAGES.to_string(),
            ));
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
            self.handle_create_cart(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("AddItem") {
            self.handle_add_item(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("UpdateQuantity") {
            self.handle_update_quantity(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("RemoveItem") {
            self.handle_remove_item(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ApplyCoupon") {
            self.handle_apply_coupon(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ClearCart") {
            self.handle_clear_cart(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("Checkout") {
            self.handle_checkout(cb, &command_any.value, &state, next_seq)?
        } else {
            return Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            )));
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        domain == self.domain
    }

    fn domains(&self) -> Vec<String> {
        vec![self.domain.clone()]
    }
}