//! Cart bounded context business logic.
//!
//! Handles shopping cart lifecycle including item management and checkout.

mod business_logic_client;
pub mod handlers;
pub mod state;

use common::{BusinessError, Result};
use angzarr::proto::{CommandBook, EventBook};
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
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Cart aggregate.
pub struct CartLogic {
    domain: String,
}

impl CartLogic {
    pub const DOMAIN: &'static str = "cart";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }
}

impl Default for CartLogic {
    fn default() -> Self {
        Self::new()
    }
}

// Public test methods for cucumber tests
impl CartLogic {
    pub fn rebuild_state_public(&self, event_book: Option<&EventBook>) -> CartState {
        rebuild_state(event_book)
    }

    pub fn handle_create_cart_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_create_cart(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_add_item_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_add_item(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_update_quantity_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_update_quantity(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_remove_item_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_remove_item(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_apply_coupon_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_apply_coupon(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_clear_cart_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_clear_cart(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_checkout_public(
        &self,
        command_book: &CommandBook,
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_checkout(command_book, &command_any.value, state, next_seq)
    }
}
