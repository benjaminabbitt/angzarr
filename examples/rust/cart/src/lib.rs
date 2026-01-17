//! Cart bounded context business logic.
//!
//! Handles shopping cart lifecycle including item management and checkout.

use prost::Message;

use angzarr::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use angzarr::proto::{
    event_page::Sequence, CommandBook,
    EventBook, EventPage,
};
use common::proto::{
    AddItem, ApplyCoupon, CartCheckedOut, CartCleared, CartCreated, CartItem, CartState,
    CouponApplied, CreateCart, ItemAdded, ItemRemoved, QuantityUpdated, RemoveItem, UpdateQuantity,
};

mod business_logic_client;

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

    /// Rebuild cart state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> CartState {
        let mut state = CartState::default();

        let Some(book) = event_book else {
            return state;
        };

        // Start from snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = CartState::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.ends_with("CartCreated") {
                if let Ok(e) = CartCreated::decode(event.value.as_slice()) {
                    state.customer_id = e.customer_id;
                    state.items.clear();
                    state.subtotal_cents = 0;
                    state.coupon_code = String::new();
                    state.discount_cents = 0;
                    state.status = "active".to_string();
                }
            } else if event.type_url.ends_with("ItemAdded") {
                if let Ok(e) = ItemAdded::decode(event.value.as_slice()) {
                    // Check if item already exists
                    if let Some(existing) = state
                        .items
                        .iter_mut()
                        .find(|i| i.product_id == e.product_id)
                    {
                        existing.quantity = e.quantity;
                    } else {
                        state.items.push(CartItem {
                            product_id: e.product_id,
                            name: e.name,
                            quantity: e.quantity,
                            unit_price_cents: e.unit_price_cents,
                        });
                    }
                    state.subtotal_cents = e.new_subtotal;
                }
            } else if event.type_url.ends_with("QuantityUpdated") {
                if let Ok(e) = QuantityUpdated::decode(event.value.as_slice()) {
                    if let Some(item) = state
                        .items
                        .iter_mut()
                        .find(|i| i.product_id == e.product_id)
                    {
                        item.quantity = e.new_quantity;
                    }
                    state.subtotal_cents = e.new_subtotal;
                }
            } else if event.type_url.ends_with("ItemRemoved") {
                if let Ok(e) = ItemRemoved::decode(event.value.as_slice()) {
                    state.items.retain(|i| i.product_id != e.product_id);
                    state.subtotal_cents = e.new_subtotal;
                }
            } else if event.type_url.ends_with("CouponApplied") {
                if let Ok(e) = CouponApplied::decode(event.value.as_slice()) {
                    state.coupon_code = e.coupon_code;
                    state.discount_cents = e.discount_cents;
                }
            } else if event.type_url.ends_with("CartCleared") {
                if let Ok(e) = CartCleared::decode(event.value.as_slice()) {
                    state.items.clear();
                    state.subtotal_cents = e.new_subtotal;
                    state.coupon_code = String::new();
                    state.discount_cents = 0;
                }
            } else if event.type_url.ends_with("CartCheckedOut") {
                state.status = "checked_out".to_string();
            }
        }

        state
    }

    fn calculate_subtotal(&self, items: &[CartItem]) -> i32 {
        items.iter().map(|i| i.quantity * i.unit_price_cents).sum()
    }

    fn handle_create_cart(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if !state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_EXISTS.to_string()));
        }

        let cmd =
            CreateCart::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = CartCreated {
            customer_id: cmd.customer_id.clone(),
            created_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: cmd.customer_id,
            items: vec![],
            subtotal_cents: 0,
            coupon_code: String::new(),
            discount_cents: 0,
            status: "active".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CartCreated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_add_item(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
        }
        if state.status == "checked_out" {
            return Err(BusinessError::Rejected(
                errmsg::CART_CHECKED_OUT.to_string(),
            ));
        }

        let cmd =
            AddItem::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.quantity <= 0 {
            return Err(BusinessError::Rejected(
                errmsg::QUANTITY_POSITIVE.to_string(),
            ));
        }

        // Calculate new quantity (add to existing if present)
        let existing_qty = state
            .items
            .iter()
            .find(|i| i.product_id == cmd.product_id)
            .map(|i| i.quantity)
            .unwrap_or(0);
        let new_quantity = existing_qty + cmd.quantity;

        // Calculate new subtotal
        let mut items = state.items.clone();
        if let Some(item) = items.iter_mut().find(|i| i.product_id == cmd.product_id) {
            item.quantity = new_quantity;
        } else {
            items.push(CartItem {
                product_id: cmd.product_id.clone(),
                name: cmd.name.clone(),
                quantity: cmd.quantity,
                unit_price_cents: cmd.unit_price_cents,
            });
        }
        let new_subtotal = self.calculate_subtotal(&items);

        let event = ItemAdded {
            product_id: cmd.product_id.clone(),
            name: cmd.name.clone(),
            quantity: new_quantity,
            unit_price_cents: cmd.unit_price_cents,
            new_subtotal,
            added_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: state.customer_id.clone(),
            items,
            subtotal_cents: new_subtotal,
            coupon_code: state.coupon_code.clone(),
            discount_cents: state.discount_cents,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ItemAdded".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_update_quantity(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
        }
        if state.status == "checked_out" {
            return Err(BusinessError::Rejected(
                errmsg::CART_CHECKED_OUT.to_string(),
            ));
        }

        let cmd = UpdateQuantity::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.new_quantity <= 0 {
            return Err(BusinessError::Rejected(
                errmsg::QUANTITY_POSITIVE.to_string(),
            ));
        }

        let item = state
            .items
            .iter()
            .find(|i| i.product_id == cmd.product_id)
            .ok_or_else(|| BusinessError::Rejected(errmsg::ITEM_NOT_IN_CART.to_string()))?;

        let old_quantity = item.quantity;

        // Calculate new subtotal
        let mut items = state.items.clone();
        if let Some(i) = items.iter_mut().find(|i| i.product_id == cmd.product_id) {
            i.quantity = cmd.new_quantity;
        }
        let new_subtotal = self.calculate_subtotal(&items);

        let event = QuantityUpdated {
            product_id: cmd.product_id,
            old_quantity,
            new_quantity: cmd.new_quantity,
            new_subtotal,
            updated_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: state.customer_id.clone(),
            items,
            subtotal_cents: new_subtotal,
            coupon_code: state.coupon_code.clone(),
            discount_cents: state.discount_cents,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.QuantityUpdated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_remove_item(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
        }
        if state.status == "checked_out" {
            return Err(BusinessError::Rejected(
                errmsg::CART_CHECKED_OUT.to_string(),
            ));
        }

        let cmd =
            RemoveItem::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let item = state
            .items
            .iter()
            .find(|i| i.product_id == cmd.product_id)
            .ok_or_else(|| BusinessError::Rejected(errmsg::ITEM_NOT_IN_CART.to_string()))?;

        let removed_quantity = item.quantity;

        // Calculate new subtotal
        let items: Vec<CartItem> = state
            .items
            .iter()
            .filter(|i| i.product_id != cmd.product_id)
            .cloned()
            .collect();
        let new_subtotal = self.calculate_subtotal(&items);

        let event = ItemRemoved {
            product_id: cmd.product_id,
            quantity: removed_quantity,
            new_subtotal,
            removed_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: state.customer_id.clone(),
            items,
            subtotal_cents: new_subtotal,
            coupon_code: state.coupon_code.clone(),
            discount_cents: state.discount_cents,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ItemRemoved".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_apply_coupon(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
        }
        if state.status == "checked_out" {
            return Err(BusinessError::Rejected(
                errmsg::CART_CHECKED_OUT.to_string(),
            ));
        }
        if !state.coupon_code.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::COUPON_ALREADY_APPLIED.to_string(),
            ));
        }
        if state.items.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_EMPTY.to_string()));
        }

        let cmd = ApplyCoupon::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let discount_cents = if cmd.coupon_type == "percentage" {
            (state.subtotal_cents * cmd.value) / 100
        } else {
            // fixed
            cmd.value
        };

        let event = CouponApplied {
            coupon_code: cmd.code.clone(),
            coupon_type: cmd.coupon_type.clone(),
            value: cmd.value,
            discount_cents,
            applied_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            coupon_code: cmd.code,
            discount_cents,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CouponApplied".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_clear_cart(
        &self,
        command_book: &CommandBook,
        _command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
        }
        if state.status == "checked_out" {
            return Err(BusinessError::Rejected(
                errmsg::CART_CHECKED_OUT.to_string(),
            ));
        }

        let event = CartCleared {
            new_subtotal: 0,
            cleared_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: state.customer_id.clone(),
            items: vec![],
            subtotal_cents: 0,
            coupon_code: String::new(),
            discount_cents: 0,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CartCleared".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_checkout(
        &self,
        command_book: &CommandBook,
        _command_data: &[u8],
        state: &CartState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_NOT_FOUND.to_string()));
        }
        if state.status == "checked_out" {
            return Err(BusinessError::Rejected(
                errmsg::CART_CHECKED_OUT.to_string(),
            ));
        }
        if state.items.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CART_EMPTY.to_string()));
        }

        let event = CartCheckedOut {
            final_subtotal: state.subtotal_cents,
            discount_cents: state.discount_cents,
            checked_out_at: Some(now()),
        };

        let new_state = CartState {
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            coupon_code: state.coupon_code.clone(),
            discount_cents: state.discount_cents,
            status: "checked_out".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CartCheckedOut".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CartState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
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
        self.rebuild_state(event_book)
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
        self.handle_create_cart(command_book, &command_any.value, state, next_seq)
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
        self.handle_add_item(command_book, &command_any.value, state, next_seq)
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
        self.handle_update_quantity(command_book, &command_any.value, state, next_seq)
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
        self.handle_remove_item(command_book, &command_any.value, state, next_seq)
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
        self.handle_apply_coupon(command_book, &command_any.value, state, next_seq)
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
        self.handle_clear_cart(command_book, &command_any.value, state, next_seq)
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
        self.handle_checkout(command_book, &command_any.value, state, next_seq)
    }
}

fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}
