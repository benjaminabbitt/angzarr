//! Order bounded context business logic.
//!
//! Handles order lifecycle, loyalty discounts, and payment processing.

use async_trait::async_trait;
use prost::Message;

use angzarr::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, ContextualCommand,
    EventBook, EventPage,
};
use common::next_sequence;
use common::proto::{
    ApplyLoyaltyDiscount, CancelOrder, ConfirmPayment, CreateOrder, LoyaltyDiscountApplied,
    OrderCancelled, OrderCompleted, OrderCreated, OrderState, PaymentSubmitted, SubmitPayment,
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

    /// Rebuild order state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> OrderState {
        let mut state = OrderState::default();

        let Some(book) = event_book else {
            return state;
        };

        // Start from snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = OrderState::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.ends_with("OrderCreated") {
                if let Ok(e) = OrderCreated::decode(event.value.as_slice()) {
                    state.customer_id = e.customer_id;
                    state.items = e.items;
                    state.subtotal_cents = e.subtotal_cents;
                    state.discount_cents = 0;
                    state.loyalty_points_used = 0;
                    state.status = "pending".to_string();
                }
            } else if event.type_url.ends_with("LoyaltyDiscountApplied") {
                if let Ok(e) = LoyaltyDiscountApplied::decode(event.value.as_slice()) {
                    state.loyalty_points_used = e.points_used;
                    state.discount_cents = e.discount_cents;
                }
            } else if event.type_url.ends_with("PaymentSubmitted") {
                if let Ok(e) = PaymentSubmitted::decode(event.value.as_slice()) {
                    state.payment_method = e.payment_method;
                    state.status = "payment_submitted".to_string();
                }
            } else if event.type_url.ends_with("OrderCompleted") {
                if let Ok(e) = OrderCompleted::decode(event.value.as_slice()) {
                    state.payment_reference = e.payment_reference;
                    state.status = "completed".to_string();
                }
            } else if event.type_url.ends_with("OrderCancelled") {
                state.status = "cancelled".to_string();
            }
        }

        state
    }

    fn calculate_total(&self, state: &OrderState) -> i32 {
        state.subtotal_cents - state.discount_cents
    }

    fn handle_create_order(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if !state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ORDER_EXISTS.to_string()));
        }

        let cmd = CreateOrder::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.items.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ITEMS_REQUIRED.to_string()));
        }

        for item in &cmd.items {
            if item.quantity <= 0 {
                return Err(BusinessError::Rejected(
                    errmsg::QUANTITY_POSITIVE.to_string(),
                ));
            }
        }

        let subtotal: i32 = cmd
            .items
            .iter()
            .map(|i| i.quantity * i.unit_price_cents)
            .sum();

        let event = OrderCreated {
            customer_id: cmd.customer_id.clone(),
            items: cmd.items.clone(),
            subtotal_cents: subtotal,
            created_at: Some(now()),
        };

        let new_state = OrderState {
            customer_id: cmd.customer_id,
            items: cmd.items,
            subtotal_cents: subtotal,
            discount_cents: 0,
            loyalty_points_used: 0,
            payment_method: String::new(),
            payment_reference: String::new(),
            status: "pending".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.OrderCreated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.OrderState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_apply_loyalty_discount(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
        }
        if state.loyalty_points_used > 0 {
            return Err(BusinessError::Rejected(
                errmsg::LOYALTY_ALREADY_APPLIED.to_string(),
            ));
        }

        let cmd = ApplyLoyaltyDiscount::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = LoyaltyDiscountApplied {
            points_used: cmd.points,
            discount_cents: cmd.discount_cents,
            applied_at: Some(now()),
        };

        let new_state = OrderState {
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            discount_cents: cmd.discount_cents,
            loyalty_points_used: cmd.points,
            payment_method: state.payment_method.clone(),
            payment_reference: state.payment_reference.clone(),
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.LoyaltyDiscountApplied".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.OrderState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_submit_payment(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
        }
        if state.status == "payment_submitted" || state.status == "completed" {
            return Err(BusinessError::Rejected(
                errmsg::PAYMENT_ALREADY_SUBMITTED.to_string(),
            ));
        }
        if state.status == "cancelled" {
            return Err(BusinessError::Rejected(errmsg::ORDER_CANCELLED.to_string()));
        }

        let cmd = SubmitPayment::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let expected_total = self.calculate_total(state);
        if cmd.amount_cents != expected_total {
            return Err(BusinessError::Rejected(format!(
                "{}: expected {}, got {}",
                errmsg::PAYMENT_AMOUNT_MISMATCH,
                expected_total,
                cmd.amount_cents
            )));
        }

        let event = PaymentSubmitted {
            payment_method: cmd.payment_method.clone(),
            amount_cents: cmd.amount_cents,
            submitted_at: Some(now()),
        };

        let new_state = OrderState {
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            discount_cents: state.discount_cents,
            loyalty_points_used: state.loyalty_points_used,
            payment_method: cmd.payment_method,
            payment_reference: state.payment_reference.clone(),
            status: "payment_submitted".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.PaymentSubmitted".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.OrderState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_confirm_payment(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
        }
        if state.status == "pending" {
            return Err(BusinessError::Rejected(
                errmsg::PAYMENT_NOT_SUBMITTED.to_string(),
            ));
        }
        if state.status == "completed" {
            return Err(BusinessError::Rejected(errmsg::ORDER_COMPLETED.to_string()));
        }
        if state.status == "cancelled" {
            return Err(BusinessError::Rejected(errmsg::ORDER_CANCELLED.to_string()));
        }

        let cmd = ConfirmPayment::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let final_total = self.calculate_total(state);
        // 1 loyalty point per $1 (100 cents)
        let loyalty_points_earned = final_total / 100;

        let event = OrderCompleted {
            final_total_cents: final_total,
            payment_method: state.payment_method.clone(),
            payment_reference: cmd.payment_reference.clone(),
            loyalty_points_earned,
            completed_at: Some(now()),
        };

        let new_state = OrderState {
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            discount_cents: state.discount_cents,
            loyalty_points_used: state.loyalty_points_used,
            payment_method: state.payment_method.clone(),
            payment_reference: cmd.payment_reference,
            status: "completed".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.OrderCompleted".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.OrderState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_cancel_order(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &OrderState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.customer_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
        }
        if state.status == "completed" {
            return Err(BusinessError::Rejected(errmsg::ORDER_COMPLETED.to_string()));
        }
        if state.status == "cancelled" {
            return Err(BusinessError::Rejected(errmsg::ORDER_CANCELLED.to_string()));
        }

        let cmd = CancelOrder::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = OrderCancelled {
            reason: cmd.reason,
            cancelled_at: Some(now()),
            loyalty_points_used: state.loyalty_points_used,
        };

        let new_state = OrderState {
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            discount_cents: state.discount_cents,
            loyalty_points_used: state.loyalty_points_used,
            payment_method: state.payment_method.clone(),
            payment_reference: state.payment_reference.clone(),
            status: "cancelled".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.OrderCancelled".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.OrderState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
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
        self.rebuild_state(event_book)
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
        self.handle_create_order(command_book, &command_any.value, state, next_seq)
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
        self.handle_apply_loyalty_discount(command_book, &command_any.value, state, next_seq)
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
        self.handle_submit_payment(command_book, &command_any.value, state, next_seq)
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
        self.handle_confirm_payment(command_book, &command_any.value, state, next_seq)
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
        self.handle_cancel_order(command_book, &command_any.value, state, next_seq)
    }
}

#[async_trait]
impl BusinessLogicClient for OrderLogic {
    async fn handle(&self, _domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);
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
            self.handle_create_order(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ApplyLoyaltyDiscount") {
            self.handle_apply_loyalty_discount(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("SubmitPayment") {
            self.handle_submit_payment(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ConfirmPayment") {
            self.handle_confirm_payment(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("CancelOrder") {
            self.handle_cancel_order(cb, &command_any.value, &state, next_seq)?
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

fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{CommandPage, Cover, Uuid as ProtoUuid};
    use common::proto::LineItem;

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
                synchronous: false,
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
