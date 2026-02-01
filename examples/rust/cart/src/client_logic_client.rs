//! AggregateLogic implementation for CartLogic.

use angzarr::proto::ContextualCommand;
use common::{dispatch_aggregate, unknown_command, AggregateLogic};

use crate::handlers::{
    handle_add_item, handle_apply_coupon, handle_checkout, handle_clear_cart, handle_create_cart,
    handle_remove_item, handle_update_quantity,
};
use crate::state::rebuild_state;
use crate::CartLogic;

#[tonic::async_trait]
impl AggregateLogic for CartLogic {
    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<angzarr::proto::BusinessResponse, tonic::Status> {
        dispatch_aggregate(cmd, rebuild_state, |cb, command_any, state, next_seq| {
            if command_any.type_url.ends_with("CreateCart") {
                handle_create_cart(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("AddItem") {
                handle_add_item(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("UpdateQuantity") {
                handle_update_quantity(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("RemoveItem") {
                handle_remove_item(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("ApplyCoupon") {
                handle_apply_coupon(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("ClearCart") {
                handle_clear_cart(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("Checkout") {
                handle_checkout(cb, &command_any.value, state, next_seq)
            } else {
                Err(unknown_command(&command_any.type_url))
            }
        })
    }
}
