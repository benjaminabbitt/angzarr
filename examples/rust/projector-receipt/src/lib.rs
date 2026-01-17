//! Receipt projector business logic.
//!
//! Generates Receipt projections from completed order events.

use prost::Message;

use angzarr::proto::{EventBook, Projection};
#[cfg(test)]
use common::proto::OrderCompleted;
use common::proto::{LineItem, LoyaltyDiscountApplied, OrderCreated, PaymentSubmitted, Receipt};

pub mod logmsg {
    pub const GENERATED_RECEIPT: &str = "generated_receipt";
}

const PROJECTOR_NAME: &str = "receipt";
const POINTS_PER_DOLLAR: i32 = 10;

/// Internal state for building the receipt.
#[derive(Default)]
struct ReceiptState {
    customer_id: String,
    items: Vec<LineItem>,
    subtotal_cents: i32,
    discount_cents: i32,
    loyalty_points_used: i32,
    final_total_cents: i32,
    payment_method: String,
    completed: bool,
}

/// Receipt projector logic.
pub struct ReceiptProjectorLogic;

impl ReceiptProjectorLogic {
    pub fn new() -> Self {
        Self
    }

    /// Project events into a Receipt if the order is completed.
    pub fn project(&self, event_book: &EventBook) -> Option<Projection> {
        if event_book.pages.is_empty() {
            return None;
        }

        let state = self.rebuild_state(event_book);
        if !state.completed {
            return None;
        }

        let order_id = event_book
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| hex::encode(&r.value))
            .unwrap_or_default();

        let loyalty_points_earned = (state.final_total_cents / 100) * POINTS_PER_DOLLAR;

        let receipt = Receipt {
            order_id: order_id.clone(),
            customer_id: state.customer_id.clone(),
            items: state.items.clone(),
            subtotal_cents: state.subtotal_cents,
            discount_cents: state.discount_cents,
            final_total_cents: state.final_total_cents,
            payment_method: state.payment_method.clone(),
            loyalty_points_earned,
            completed_at: None,
            formatted_text: self.format_receipt(&order_id, &state, loyalty_points_earned),
        };

        let sequence = event_book
            .pages
            .last()
            .and_then(|p| match &p.sequence {
                Some(angzarr::proto::event_page::Sequence::Num(n)) => Some(*n),
                _ => None,
            })
            .unwrap_or(0);

        Some(Projection {
            cover: event_book.cover.clone(),
            projector: PROJECTOR_NAME.to_string(),
            sequence,
            projection: Some(prost_types::Any {
                type_url: "type.examples/examples.Receipt".to_string(),
                value: receipt.encode_to_vec(),
            }),
        })
    }

    fn rebuild_state(&self, event_book: &EventBook) -> ReceiptState {
        let mut state = ReceiptState::default();

        for page in &event_book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.ends_with("OrderCreated") {
                if let Ok(e) = OrderCreated::decode(event.value.as_slice()) {
                    state.customer_id = e.customer_id;
                    state.items = e.items;
                    state.subtotal_cents = e.subtotal_cents;
                }
            } else if event.type_url.ends_with("LoyaltyDiscountApplied") {
                if let Ok(e) = LoyaltyDiscountApplied::decode(event.value.as_slice()) {
                    state.loyalty_points_used = e.points_used;
                    state.discount_cents += e.discount_cents;
                }
            } else if event.type_url.ends_with("PaymentSubmitted") {
                if let Ok(e) = PaymentSubmitted::decode(event.value.as_slice()) {
                    state.payment_method = e.payment_method;
                    state.final_total_cents = e.amount_cents;
                }
            } else if event.type_url.ends_with("OrderCompleted") {
                state.completed = true;
            }
        }

        state
    }

    fn format_receipt(
        &self,
        order_id: &str,
        state: &ReceiptState,
        loyalty_points_earned: i32,
    ) -> String {
        let line = "═".repeat(40);
        let thin_line = "─".repeat(40);

        let short_order_id = if order_id.len() > 16 {
            &order_id[..16]
        } else {
            order_id
        };
        let short_cust_id = if state.customer_id.len() > 16 {
            &state.customer_id[..16]
        } else {
            &state.customer_id
        };

        let mut lines = Vec::new();
        lines.push(line.clone());
        lines.push("           RECEIPT".to_string());
        lines.push(line.clone());
        lines.push(format!("Order: {}...", short_order_id));
        lines.push(format!(
            "Customer: {}",
            if short_cust_id.is_empty() {
                "N/A".to_string()
            } else {
                format!("{}...", short_cust_id)
            }
        ));
        lines.push(thin_line.clone());

        for item in &state.items {
            let line_total = item.quantity * item.unit_price_cents;
            lines.push(format!(
                "{} x {} @ ${:.2} = ${:.2}",
                item.quantity,
                item.name,
                item.unit_price_cents as f64 / 100.0,
                line_total as f64 / 100.0
            ));
        }

        lines.push(thin_line.clone());
        lines.push(format!(
            "Subtotal:              ${:.2}",
            state.subtotal_cents as f64 / 100.0
        ));

        if state.discount_cents > 0 {
            let discount_type = if state.loyalty_points_used > 0 {
                "loyalty"
            } else {
                "coupon"
            };
            lines.push(format!(
                "Discount ({}):       -${:.2}",
                discount_type,
                state.discount_cents as f64 / 100.0
            ));
        }

        lines.push(thin_line.clone());
        lines.push(format!(
            "TOTAL:                 ${:.2}",
            state.final_total_cents as f64 / 100.0
        ));
        lines.push(format!("Payment: {}", state.payment_method));
        lines.push(thin_line.clone());
        lines.push(format!("Loyalty Points Earned: {}", loyalty_points_earned));
        lines.push(line.clone());
        lines.push("     Thank you for your purchase!".to_string());
        lines.push(line);

        lines.join("\n")
    }
}

impl Default for ReceiptProjectorLogic {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventPage, Uuid as ProtoUuid};

    fn make_event(type_url: &str, value: Vec<u8>, seq: u32) -> EventPage {
        EventPage {
            sequence: Some(Sequence::Num(seq)),
            event: Some(prost_types::Any {
                type_url: type_url.to_string(),
                value,
            }),
            created_at: None,
            synchronous: false,
        }
    }

    fn make_event_book(pages: Vec<EventPage>) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "order".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                }),
            }),
            pages,
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    #[test]
    fn test_project_empty_book_returns_none() {
        let logic = ReceiptProjectorLogic::new();
        let book = make_event_book(vec![]);

        let result = logic.project(&book);

        assert!(result.is_none());
    }

    #[test]
    fn test_project_incomplete_order_returns_none() {
        let logic = ReceiptProjectorLogic::new();

        let order_created = OrderCreated {
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

        let book = make_event_book(vec![make_event(
            "type.examples/examples.OrderCreated",
            order_created.encode_to_vec(),
            0,
        )]);

        let result = logic.project(&book);

        assert!(result.is_none());
    }

    #[test]
    fn test_project_completed_order_generates_receipt() {
        let logic = ReceiptProjectorLogic::new();

        let order_created = OrderCreated {
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

        let payment_submitted = PaymentSubmitted {
            payment_method: "card".to_string(),
            amount_cents: 2000,
            submitted_at: None,
        };

        let order_completed = OrderCompleted {
            final_total_cents: 2000,
            payment_method: "card".to_string(),
            payment_reference: "PAY-123".to_string(),
            loyalty_points_earned: 20,
            completed_at: None,
        };

        let book = make_event_book(vec![
            make_event(
                "type.examples/examples.OrderCreated",
                order_created.encode_to_vec(),
                0,
            ),
            make_event(
                "type.examples/examples.PaymentSubmitted",
                payment_submitted.encode_to_vec(),
                1,
            ),
            make_event(
                "type.examples/examples.OrderCompleted",
                order_completed.encode_to_vec(),
                2,
            ),
        ]);

        let result = logic.project(&book);

        assert!(result.is_some());
        let projection = result.unwrap();
        assert_eq!(projection.projector, "receipt");
        assert_eq!(projection.sequence, 2);

        let receipt_any = projection.projection.unwrap();
        let receipt = Receipt::decode(receipt_any.value.as_slice()).unwrap();
        assert_eq!(receipt.customer_id, "CUST-001");
        assert_eq!(receipt.final_total_cents, 2000);
        assert_eq!(receipt.loyalty_points_earned, 200); // 2000 cents = $20 = 200 points
        assert!(receipt.formatted_text.contains("RECEIPT"));
    }

    #[test]
    fn test_project_with_loyalty_discount() {
        let logic = ReceiptProjectorLogic::new();

        let order_created = OrderCreated {
            customer_id: "CUST-002".to_string(),
            items: vec![LineItem {
                product_id: "SKU-002".to_string(),
                name: "Gadget".to_string(),
                quantity: 1,
                unit_price_cents: 5000,
            }],
            subtotal_cents: 5000,
            created_at: None,
        };

        let discount_applied = LoyaltyDiscountApplied {
            points_used: 100,
            discount_cents: 1000,
            applied_at: None,
        };

        let payment_submitted = PaymentSubmitted {
            payment_method: "cash".to_string(),
            amount_cents: 4000,
            submitted_at: None,
        };

        let order_completed = OrderCompleted {
            final_total_cents: 4000,
            payment_method: "cash".to_string(),
            payment_reference: "PAY-456".to_string(),
            loyalty_points_earned: 40,
            completed_at: None,
        };

        let book = make_event_book(vec![
            make_event(
                "type.examples/examples.OrderCreated",
                order_created.encode_to_vec(),
                0,
            ),
            make_event(
                "type.examples/examples.LoyaltyDiscountApplied",
                discount_applied.encode_to_vec(),
                1,
            ),
            make_event(
                "type.examples/examples.PaymentSubmitted",
                payment_submitted.encode_to_vec(),
                2,
            ),
            make_event(
                "type.examples/examples.OrderCompleted",
                order_completed.encode_to_vec(),
                3,
            ),
        ]);

        let result = logic.project(&book);

        assert!(result.is_some());
        let projection = result.unwrap();
        let receipt_any = projection.projection.unwrap();
        let receipt = Receipt::decode(receipt_any.value.as_slice()).unwrap();

        assert_eq!(receipt.subtotal_cents, 5000);
        assert_eq!(receipt.discount_cents, 1000);
        assert_eq!(receipt.final_total_cents, 4000);
        assert!(receipt.formatted_text.contains("loyalty"));
    }
}
