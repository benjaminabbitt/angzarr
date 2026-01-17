//! Command handlers for the Customer aggregate.

mod add_loyalty_points;
mod create_customer;
mod redeem_loyalty_points;

pub use add_loyalty_points::handle_add_loyalty_points;
pub use create_customer::handle_create_customer;
pub use redeem_loyalty_points::handle_redeem_loyalty_points;

use crate::now;

/// Type URL prefix for customer events.
pub const EVENT_TYPE_PREFIX: &str = "type.examples/examples.";

/// Type URL for CustomerState.
pub const CUSTOMER_STATE_TYPE_URL: &str = "type.examples/examples.CustomerState";
