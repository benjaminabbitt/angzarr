//! Cart bounded context business logic.
//!
//! Handles shopping cart lifecycle including item management and checkout.

mod business_logic_client;
pub mod handlers;
pub mod state;

use common::proto::CartState;

// Re-export handlers for external use
pub use handlers::{
    handle_add_item, handle_apply_coupon, handle_checkout, handle_clear_cart, handle_create_cart,
    handle_remove_item, handle_update_quantity,
};
pub use state::rebuild_state;

pub mod errmsg {
    pub const CART_EXISTS: &str = "Cart already exists";
    pub const CART_NOT_FOUND: &str = "Cart does not exist";
    pub const CART_CHECKED_OUT: &str = "Cart is already checked out";
    pub const CART_EMPTY: &str = "Cart is empty";
    pub const ITEM_NOT_IN_CART: &str = "Item not in cart";
    pub const QUANTITY_POSITIVE: &str = "Quantity must be positive";
    pub const COUPON_ALREADY_APPLIED: &str = "Coupon already applied";
}

/// Business logic for Cart aggregate.
pub struct CartLogic;

common::define_aggregate!(CartLogic, "cart");

common::expose_handlers!(fns, CartLogic, CartState, rebuild: rebuild_state, [
    (handle_create_cart_public, handle_create_cart),
    (handle_add_item_public, handle_add_item),
    (handle_update_quantity_public, handle_update_quantity),
    (handle_remove_item_public, handle_remove_item),
    (handle_apply_coupon_public, handle_apply_coupon),
    (handle_clear_cart_public, handle_clear_cart),
    (handle_checkout_public, handle_checkout),
]);
