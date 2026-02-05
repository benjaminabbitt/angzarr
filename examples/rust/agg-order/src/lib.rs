//! Order bounded context client logic.
//!
//! Handles order lifecycle, loyalty discounts, and payment processing.
//!
//! ## External Service Integration
//!
//! This aggregate demonstrates calling external services during command handling.
//! The fraud service is just one example - aggregates may call ANY external REST/gRPC service:
//! - Pricing services (dynamic pricing, MSRP lookup)
//! - Tax services (calculate taxes by jurisdiction)
//! - Address validation services
//! - Payment gateways (pre-authorization, card verification)
//! - Customer services (loyalty status, preferences)
//! - Analytics/ML services (recommendations, predictions)
//! - Inventory services (real-time availability)
//! - Notification services (send confirmations)
//!
//! Configure via `FRAUD_SERVICE_URL` environment variable or `with_fraud_service_url()`.
//!
//! **Note:** Aggregates are the correct place to pull in externalities. The aggregate
//! (or command generation) is where external service calls should happen - before
//! events are emitted with enriched data.

pub mod fraud_client;
mod handlers;
mod state;

use std::sync::Arc;

use angzarr::proto::{business_response, BusinessResponse, ComponentDescriptor, ContextualCommand};
use common::proto::{ConfirmPayment, OrderCompleted, OrderState};
use common::{decode_command, extract_command, next_sequence, now, require_exists, require_status_not, AggregateLogic, CommandRouter};
use tracing::info;

pub use fraud_client::{FraudCheckResult, FraudError, FraudServiceClient};

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
    pub const ORDER_NOT_PENDING: &str = "Order is not in pending state";
    pub const POINTS_POSITIVE: &str = "Points must be positive";
    pub const DISCOUNT_POSITIVE: &str = "Discount must be positive";
    pub const DISCOUNT_EXCEEDS_SUBTOTAL: &str = "Discount cannot exceed subtotal";
    pub const PAYMENT_AMOUNT_MISMATCH: &str = "Payment amount does not match order total";
    pub const FRAUD_CHECK_DECLINED: &str = "Payment declined by fraud check";
    pub use common::errmsg::*;
}

/// Client logic for Order aggregate.
///
/// Optionally integrates with an external fraud check service.
/// Configure via environment variable `FRAUD_SERVICE_URL` or constructor.
///
/// **Note:** Aggregates are the correct place to pull in externalities. External
/// service calls (fraud check, pricing, tax, etc.) should happen here, enriching
/// the emitted events with data from external sources.
pub struct OrderLogic {
    router: CommandRouter<OrderState>,
    fraud_client: Option<Arc<FraudServiceClient>>,
}

impl OrderLogic {
    pub const DOMAIN: &'static str = "order";

    /// Create a new OrderLogic without external service integration.
    pub fn new() -> Self {
        Self::with_fraud_service_url(None)
    }

    /// Create a new OrderLogic with an optional fraud service URL.
    ///
    /// # Arguments
    /// * `url` - Base URL of the fraud service (e.g., "http://fraud-service:8080")
    ///           If None, fraud check always returns Approved.
    pub fn with_fraud_service_url(url: Option<&str>) -> Self {
        Self {
            router: CommandRouter::new("order", rebuild_state)
                .on("CreateOrder", handle_create_order)
                .on("ApplyLoyaltyDiscount", handle_apply_loyalty_discount)
                .on("SubmitPayment", handle_submit_payment)
                // ConfirmPayment is handled specially in dispatch to call fraud service
                .on("CancelOrder", handle_cancel_order),
            fraud_client: url.map(|u| Arc::new(FraudServiceClient::new(u))),
        }
    }

    /// Create from environment variable `FRAUD_SERVICE_URL`.
    pub fn from_env() -> Self {
        let url = std::env::var("FRAUD_SERVICE_URL").ok();
        Self::with_fraud_service_url(url.as_deref())
    }

    /// Handle ConfirmPayment with fraud check.
    ///
    /// This demonstrates calling an external service during command handling.
    /// The fraud result is incorporated into the emitted OrderCompleted event.
    async fn handle_confirm_payment_with_fraud(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        let command_book = cmd.command.as_ref().ok_or_else(|| {
            tonic::Status::invalid_argument("missing command")
        })?;
        let prior_events = cmd.events.as_ref();

        // Rebuild state from prior events
        let state = rebuild_state(prior_events);
        let next_seq = next_sequence(prior_events);

        // Validate preconditions
        require_exists(&state.customer_id, errmsg::ORDER_NOT_FOUND)
            .map_err(|e| tonic::Status::failed_precondition(e.to_string()))?;
        require_status_not(&state.status, "pending", errmsg::PAYMENT_NOT_SUBMITTED)
            .map_err(|e| tonic::Status::failed_precondition(e.to_string()))?;
        require_status_not(&state.status, "completed", errmsg::ORDER_COMPLETED)
            .map_err(|e| tonic::Status::failed_precondition(e.to_string()))?;
        require_status_not(&state.status, "cancelled", errmsg::ORDER_CANCELLED)
            .map_err(|e| tonic::Status::failed_precondition(e.to_string()))?;

        // Call external fraud service if configured
        let fraud_result = if let Some(ref client) = self.fraud_client {
            let total = calculate_total(&state);
            info!(
                customer_id = %state.customer_id,
                amount_cents = total,
                payment_method = %state.payment_method,
                "calling fraud service"
            );
            client
                .check(&state.customer_id, total, &state.payment_method)
                .await
                .map_err(|e| tonic::Status::unavailable(format!("fraud service: {}", e)))?
        } else {
            // No fraud service configured - approve by default
            FraudCheckResult::Approved
        };

        info!(fraud_check_result = %fraud_result, "fraud check complete");

        // Decline if fraud check failed
        if fraud_result == FraudCheckResult::Declined {
            return Err(tonic::Status::failed_precondition(errmsg::FRAUD_CHECK_DECLINED));
        }

        // Extract command data
        let command_any = extract_command(command_book)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        let confirm_cmd: ConfirmPayment = decode_command(&command_any.value)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;

        let final_total = calculate_total(&state);
        let loyalty_points_earned = final_total / 100;

        let event = OrderCompleted {
            final_total_cents: final_total,
            payment_method: state.payment_method.clone(),
            payment_reference: confirm_cmd.payment_reference.clone(),
            loyalty_points_earned,
            completed_at: Some(now()),
            customer_root: state.customer_root.clone(),
            cart_root: state.cart_root.clone(),
            items: state.items.clone(),
            fraud_check_result: fraud_result.to_string(),
        };

        let events = state::build_event_response(
            &state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.OrderCompleted",
            event,
        );

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
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
        // Extract command type from the first page
        let command_type = cmd.command.as_ref()
            .and_then(|c| c.pages.first())
            .and_then(|p| p.command.as_ref())
            .map(|c| c.type_url.as_str())
            .unwrap_or("");

        // Special handling for ConfirmPayment to call fraud service
        if command_type.ends_with("ConfirmPayment") {
            return self.handle_confirm_payment_with_fraud(cmd).await;
        }

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
