//! Order bounded context business logic.
//!
//! Handles order lifecycle, loyalty discounts, and payment processing.

mod handlers;
mod state;

use async_trait::async_trait;

use angzarr::clients::{BusinessError, BusinessLogicClient, Result};
use angzarr::proto::{
    business_response, BusinessResponse, CommandBook, ContextualCommand, EventBook,
};
use common::next_sequence;
use common::proto::OrderState;

// Re-export state functions for tests and external use
pub use state::{calculate_total, rebuild_state};

// Re-export handlers for tests and external use
pub use handlers::{
    handle_apply_loyalty_discount, handle_cancel_order, handle_confirm_payment,
    handle_create_order, handle_submit_payment,
};

pub mod errmsg {
    pub const ORDER_EXISTS: &str = "Order already exists";
    pub const ORDER_NOT_FOUND: &str = "Order does not exist";
    pub const ITEMS_REQUIRED: &str = "Order must have items";
    pub const QUANTITY_POSITIVE: &str = "Item quantity must be positive";
    pub const LOYALTY_ALREADY_APPLIED: &str = "Loyalty discount already applied";
    pub const PAYMENT_ALREADY_SUBMITTED: &str = "Payment already submitted";
    pub const PAYMENT_NOT_SUBMITTED: &str = "Payment not submitted";
    pub const ORDER_COMPLETED: &str = "Order is already completed";
    pub const ORDER_CANCELLED: &str = "Order is already cancelled";
    pub const PAYMENT_AMOUNT_MISMATCH: &str = "Payment amount does not match order total";
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Order aggregate.
pub struct OrderLogic {
    domain: String,
}

impl OrderLogic {
    pub const DOMAIN: &'static str = "order";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }
}

impl Default for OrderLogic {
    fn default() -> Self {
        Self::new()
    }
}

// Public test methods for cucumber tests
impl OrderLogic {
    pub fn rebuild_state_public(&self, event_book: Option<&EventBook>) -> OrderState {
        rebuild_state(event_book)
    }

    pub fn handle_create_order_public(
        &self,
        command_book: &CommandBook,
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_create_order(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_apply_loyalty_discount_public(
        &self,
        command_book: &CommandBook,
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_apply_loyalty_discount(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_submit_payment_public(
        &self,
        command_book: &CommandBook,
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_submit_payment(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_confirm_payment_public(
        &self,
        command_book: &CommandBook,
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_confirm_payment(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_cancel_order_public(
        &self,
        command_book: &CommandBook,
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        handle_cancel_order(command_book, &command_any.value, state, next_seq)
    }
}

#[async_trait]
impl BusinessLogicClient for OrderLogic {
    async fn handle(&self, _domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = rebuild_state(prior_events);
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

        let events = if command_any.type_url.ends_with("CreateOrder") {
            handle_create_order(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ApplyLoyaltyDiscount") {
            handle_apply_loyalty_discount(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("SubmitPayment") {
            handle_submit_payment(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ConfirmPayment") {
            handle_confirm_payment(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("CancelOrder") {
            handle_cancel_order(cb, &command_any.value, &state, next_seq)?
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

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{CommandPage, Cover, Uuid as ProtoUuid};
    use common::proto::{CreateOrder, LineItem, OrderCreated};
    use prost::Message;

    fn make_command_book(domain: &str, root: &[u8], type_url: &str, value: Vec<u8>) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: type_url.to_string(),
                    value,
                }),
            }],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        }
    }

    fn extract_events(response: BusinessResponse) -> EventBook {
        match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events in response"),
        }
    }

    #[tokio::test]
    async fn test_create_order_success() {
        let logic = OrderLogic::new();

        let cmd = CreateOrder {
            customer_id: "CUST-001".to_string(),
            items: vec![
                LineItem {
                    product_id: "SKU-001".to_string(),
                    name: "Widget".to_string(),
                    quantity: 2,
                    unit_price_cents: 1000,
                },
                LineItem {
                    product_id: "SKU-002".to_string(),
                    name: "Gadget".to_string(),
                    quantity: 1,
                    unit_price_cents: 2500,
                },
            ],
        };

        let command_book = make_command_book(
            "order",
            &[1; 16],
            "type.examples/examples.CreateOrder",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle("order", ctx).await.unwrap();
        let result = extract_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            OrderCreated::decode(result.pages[0].event.as_ref().unwrap().value.as_slice()).unwrap();
        assert_eq!(event.customer_id, "CUST-001");
        assert_eq!(event.items.len(), 2);
        assert_eq!(event.subtotal_cents, 4500);
    }

    #[tokio::test]
    async fn test_create_order_empty_items() {
        let logic = OrderLogic::new();

        let cmd = CreateOrder {
            customer_id: "CUST-002".to_string(),
            items: vec![],
        };

        let command_book = make_command_book(
            "order",
            &[1; 16],
            "type.examples/examples.CreateOrder",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let result = logic.handle("order", ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("items"));
    }
}
