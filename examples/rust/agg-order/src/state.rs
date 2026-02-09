//! Order state management and reconstruction from events.

use common::proto::{
    LoyaltyDiscountApplied, OrderCancelled, OrderCompleted, OrderCreated, OrderState,
    PaymentSubmitted,
};
use common::{ProtoTypeName, StateBuilder};
use prost::Message;

use angzarr::proto::EventBook;

use crate::status::OrderStatus;

// ============================================================================
// Named event appliers
// ============================================================================

fn apply_order_created(state: &mut OrderState, event: &prost_types::Any) {
    if let Ok(e) = OrderCreated::decode(event.value.as_slice()) {
        state.customer_id = e.customer_id;
        state.items = e.items;
        state.subtotal_cents = e.subtotal_cents;
        state.discount_cents = 0;
        state.loyalty_points_used = 0;
        state.status = OrderStatus::Pending.to_string();
        state.customer_root = e.customer_root;
        state.cart_root = e.cart_root;
    }
}

fn apply_loyalty_discount(state: &mut OrderState, event: &prost_types::Any) {
    if let Ok(e) = LoyaltyDiscountApplied::decode(event.value.as_slice()) {
        state.loyalty_points_used = e.points_used;
        state.discount_cents = e.discount_cents;
    }
}

fn apply_payment_submitted(state: &mut OrderState, event: &prost_types::Any) {
    if let Ok(e) = PaymentSubmitted::decode(event.value.as_slice()) {
        state.payment_method = e.payment_method;
        state.status = OrderStatus::PaymentSubmitted.to_string();
    }
}

fn apply_order_completed(state: &mut OrderState, event: &prost_types::Any) {
    if let Ok(e) = OrderCompleted::decode(event.value.as_slice()) {
        state.payment_reference = e.payment_reference;
        state.status = OrderStatus::Completed.to_string();
    }
}

fn apply_order_cancelled(state: &mut OrderState, _event: &prost_types::Any) {
    state.status = OrderStatus::Cancelled.to_string();
}

// ============================================================================
// State rebuilding
// ============================================================================

/// Create the StateBuilder with all registered event handlers.
///
/// Single source of truth for event type → applier mapping.
pub fn state_builder() -> StateBuilder<OrderState> {
    StateBuilder::new()
        .on(OrderCreated::TYPE_NAME, apply_order_created)
        .on(LoyaltyDiscountApplied::TYPE_NAME, apply_loyalty_discount)
        .on(PaymentSubmitted::TYPE_NAME, apply_payment_submitted)
        .on(OrderCompleted::TYPE_NAME, apply_order_completed)
        .on(OrderCancelled::TYPE_NAME, apply_order_cancelled)
}

/// Rebuild order state from event history using StateBuilder.
pub fn rebuild_state(event_book: Option<&EventBook>) -> OrderState {
    state_builder().rebuild(event_book)
}

/// Calculate the final total for an order (subtotal minus discounts).
pub fn calculate_total(state: &OrderState) -> i32 {
    state.subtotal_cents - state.discount_cents
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventPage, Uuid as ProtoUuid};
    use common::proto::LineItem;

    fn make_event_book(events: Vec<(&str, Vec<u8>)>) -> EventBook {
        let pages = events
            .into_iter()
            .enumerate()
            .map(|(i, (type_url, value))| EventPage {
                sequence: Some(Sequence::Num(i as u32)),
                event: Some(prost_types::Any {
                    type_url: type_url.to_string(),
                    value,
                }),
                created_at: None,
            })
            .collect();

        EventBook {
            cover: Some(Cover {
                domain: "order".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages,
            snapshot: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_rebuild_state_empty() {
        let state = rebuild_state(None);
        assert!(state.customer_id.is_empty());
        assert_eq!(state.subtotal_cents, 0);
    }

    #[test]
    fn test_rebuild_state_from_events() {
        let created = OrderCreated {
            customer_id: "CUST-001".to_string(),
            items: vec![LineItem {
                product_id: "SKU-001".to_string(),
                name: "Widget".to_string(),
                quantity: 2,
                unit_price_cents: 1000,
                ..Default::default()
            }],
            subtotal_cents: 2000,
            created_at: None,
            ..Default::default()
        };

        let event_book = make_event_book(vec![(
            "type.examples/examples.OrderCreated",
            created.encode_to_vec(),
        )]);

        let state = rebuild_state(Some(&event_book));
        assert_eq!(state.customer_id, "CUST-001");
        assert_eq!(state.subtotal_cents, 2000);
        assert!(state.status == OrderStatus::Pending);
    }

    #[test]
    fn test_calculate_total() {
        let state = OrderState {
            customer_id: "CUST-001".to_string(),
            subtotal_cents: 5000,
            discount_cents: 500,
            ..Default::default()
        };

        assert_eq!(calculate_total(&state), 4500);
    }
}
