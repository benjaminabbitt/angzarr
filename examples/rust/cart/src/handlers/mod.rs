//! Cart command handlers.
//!
//! Each handler processes a specific cart command and produces events.

mod add_item;
mod apply_coupon;
mod checkout;
mod clear_cart;
mod create_cart;
mod remove_item;
mod update_quantity;

pub use add_item::handle_add_item;
pub use apply_coupon::handle_apply_coupon;
pub use checkout::handle_checkout;
pub use clear_cart::handle_clear_cart;
pub use create_cart::handle_create_cart;
pub use remove_item::handle_remove_item;
pub use update_quantity::handle_update_quantity;
