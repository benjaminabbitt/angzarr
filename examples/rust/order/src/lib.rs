//! Order bounded context client logic.
//!
//! Handles order lifecycle, loyalty discounts, and payment processing.

mod handlers;
mod state;

use angzarr::proto::{BusinessResponse, ComponentDescriptor, ContextualCommand};
use common::proto::OrderState;
use common::{AggregateLogic, CommandRouter};

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
    pub use common::errmsg::*;
}

/// Client logic for Order aggregate.
pub struct OrderLogic {
    router: CommandRouter<OrderState>,
}

impl OrderLogic {
    pub const DOMAIN: &'static str = "order";

    pub fn new() -> Self {
        Self {
            router: CommandRouter::new("order", rebuild_state)
                .on("CreateOrder", handle_create_order)
                .on("ApplyLoyaltyDiscount", handle_apply_loyalty_discount)
                .on("SubmitPayment", handle_submit_payment)
                .on("ConfirmPayment", handle_confirm_payment)
                .on("CancelOrder", handle_cancel_order),
        }
    }
}

impl Default for OrderLogic {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl AggregateLogic for OrderLogic {
    fn descriptor(&self) -> ComponentDescriptor {
        self.router.descriptor()
    }

    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        self.router.dispatch(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::proto::{CreateOrder, LineItem, OrderCreated};
    use common::testing::{extract_response_events, make_test_command_book};
    use prost::Message;

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
                    ..Default::default()
                },
                LineItem {
                    product_id: "SKU-002".to_string(),
                    name: "Gadget".to_string(),
                    quantity: 1,
                    unit_price_cents: 2500,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let command_book = make_test_command_book(
            "order",
            &[1; 16],
            "type.examples/examples.CreateOrder",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
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
            ..Default::default()
        };

        let command_book = make_test_command_book(
            "order",
            &[1; 16],
            "type.examples/examples.CreateOrder",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let result = logic.handle(ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("items"));
    }
}
