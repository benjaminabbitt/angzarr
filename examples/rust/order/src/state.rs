//! Order state management and reconstruction from events.

use common::proto::{
    LoyaltyDiscountApplied, OrderCompleted, OrderCreated, OrderState, PaymentSubmitted,
};
use prost::Message;

use angzarr::proto::EventBook;

/// Rebuild order state from event history.
pub fn rebuild_state(event_book: Option<&EventBook>) -> OrderState {
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
                sequence: Some(Sequence::Num(i as u32 + 1)),
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
            }),
            snapshot: None,
            pages,
            correlation_id: String::new(),
            snapshot_state: None,
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
            }],
            subtotal_cents: 2000,
            created_at: None,
        };

        let event_book = make_event_book(vec![(
            "type.examples/examples.OrderCreated",
            created.encode_to_vec(),
        )]);

        let state = rebuild_state(Some(&event_book));
        assert_eq!(state.customer_id, "CUST-001");
        assert_eq!(state.subtotal_cents, 2000);
        assert_eq!(state.status, "pending");
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
