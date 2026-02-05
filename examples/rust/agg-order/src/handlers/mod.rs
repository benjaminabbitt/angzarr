//! Command handlers for the Order aggregate.

mod apply_loyalty_discount;
mod cancel_order;
mod confirm_payment;
mod create_order;
mod submit_payment;

pub use apply_loyalty_discount::handle_apply_loyalty_discount;
pub use cancel_order::handle_cancel_order;
pub use confirm_payment::handle_confirm_payment;
pub use create_order::handle_create_order;
pub use submit_payment::handle_submit_payment;
