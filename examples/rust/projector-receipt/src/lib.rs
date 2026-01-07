//! Receipt Projector - Rust Implementation.
//!
//! Generates human-readable receipts when transactions complete.

use std::sync::Arc;

use evented::async_trait::async_trait;
use evented::interfaces::projector::{Projector, Result};
use evented::proto::{EventBook, Projection};
use prost::Message;

mod proto;
use proto::{DiscountApplied, LineItem, Receipt, TransactionCompleted, TransactionCreated};

/// Projector that generates receipts when transactions complete.
pub struct ReceiptProjector {
    name: String,
}

impl ReceiptProjector {
    /// Create a new receipt projector.
    pub fn new() -> Self {
        Self {
            name: "receipt".to_string(),
        }
    }
}

impl Default for ReceiptProjector {
    fn default() -> Self {
        Self::new()
    }
}

/// Transaction state rebuilt from events.
#[derive(Default)]
struct TransactionState {
    customer_id: String,
    items: Vec<LineItem>,
    subtotal_cents: i32,
    discount_cents: i32,
    discount_type: String,
    final_total_cents: i32,
    payment_method: String,
    loyalty_points_earned: i32,
    completed: bool,
}

#[async_trait]
impl Projector for ReceiptProjector {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        vec!["transaction".to_string()]
    }

    async fn project(&self, book: &Arc<EventBook>) -> Result<Option<Projection>> {
        // Rebuild transaction state from all events
        let mut state = TransactionState::default();

        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.contains("TransactionCreated") {
                if let Ok(created) = TransactionCreated::decode(event.value.as_slice()) {
                    state.customer_id = created.customer_id;
                    state.items = created.items;
                    state.subtotal_cents = created.subtotal_cents;
                }
            } else if event.type_url.contains("DiscountApplied") {
                if let Ok(discount) = DiscountApplied::decode(event.value.as_slice()) {
                    state.discount_type = discount.discount_type;
                    state.discount_cents = discount.discount_cents;
                }
            } else if event.type_url.contains("TransactionCompleted") {
                if let Ok(completed) = TransactionCompleted::decode(event.value.as_slice()) {
                    state.final_total_cents = completed.final_total_cents;
                    state.payment_method = completed.payment_method;
                    state.loyalty_points_earned = completed.loyalty_points_earned;
                    state.completed = true;
                }
            }
        }

        // Only generate receipt if transaction completed
        if !state.completed {
            return Ok(None);
        }

        let transaction_id = book
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| hex::encode(&r.value))
            .unwrap_or_default();

        // Generate formatted receipt text
        let receipt_text = format_receipt(&transaction_id, &state);

        println!(
            "[{}] Generated receipt for transaction {}...",
            self.name,
            &transaction_id[..16.min(transaction_id.len())]
        );

        // Create Receipt using prost
        let receipt = Receipt {
            transaction_id: transaction_id.clone(),
            customer_id: state.customer_id,
            items: state.items,
            subtotal_cents: state.subtotal_cents,
            discount_cents: state.discount_cents,
            final_total_cents: state.final_total_cents,
            payment_method: state.payment_method,
            loyalty_points_earned: state.loyalty_points_earned,
            completed_at: None,
            formatted_text: receipt_text,
        };

        let projection = Projection {
            cover: book.cover.clone(),
            projector: self.name.clone(),
            sequence: book
                .pages
                .last()
                .and_then(|p| p.sequence.as_ref())
                .map(|s| match s {
                    evented::proto::event_page::Sequence::Num(n) => *n,
                    evented::proto::event_page::Sequence::Force(_) => 0,
                })
                .unwrap_or(0),
            projection: Some(prost_types::Any {
                type_url: "type.examples/examples.Receipt".to_string(),
                value: receipt.encode_to_vec(),
            }),
        };

        Ok(Some(projection))
    }

    fn is_synchronous(&self) -> bool {
        true
    }
}

/// Format a human-readable receipt.
fn format_receipt(transaction_id: &str, state: &TransactionState) -> String {
    let mut lines = Vec::new();

    lines.push("═".repeat(40));
    lines.push("           RECEIPT".to_string());
    lines.push("═".repeat(40));
    lines.push(format!(
        "Transaction: {}...",
        &transaction_id[..16.min(transaction_id.len())]
    ));
    lines.push(format!(
        "Customer: {}...",
        &state.customer_id[..16.min(state.customer_id.len())]
    ));
    lines.push("─".repeat(40));

    // Items
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

    lines.push("─".repeat(40));
    lines.push(format!(
        "Subtotal:              ${:.2}",
        state.subtotal_cents as f64 / 100.0
    ));

    if state.discount_cents > 0 {
        lines.push(format!(
            "Discount ({}):       -${:.2}",
            state.discount_type,
            state.discount_cents as f64 / 100.0
        ));
    }

    lines.push("─".repeat(40));
    lines.push(format!(
        "TOTAL:                 ${:.2}",
        state.final_total_cents as f64 / 100.0
    ));
    lines.push(format!("Payment: {}", state.payment_method));
    lines.push("─".repeat(40));
    lines.push(format!(
        "Loyalty Points Earned: {}",
        state.loyalty_points_earned
    ));
    lines.push("═".repeat(40));
    lines.push("     Thank you for your purchase!".to_string());
    lines.push("═".repeat(40));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_receipt() {
        let state = TransactionState {
            customer_id: "cust123456789012".to_string(),
            items: vec![
                LineItem {
                    product_id: "prod1".to_string(),
                    name: "Widget".to_string(),
                    quantity: 2,
                    unit_price_cents: 999,
                },
                LineItem {
                    product_id: "prod2".to_string(),
                    name: "Gadget".to_string(),
                    quantity: 1,
                    unit_price_cents: 2499,
                },
            ],
            subtotal_cents: 4497,
            discount_cents: 500,
            discount_type: "coupon".to_string(),
            final_total_cents: 3997,
            payment_method: "card".to_string(),
            loyalty_points_earned: 39,
            completed: true,
        };

        let receipt = format_receipt("tx1234567890123456", &state);

        assert!(receipt.contains("RECEIPT"));
        assert!(receipt.contains("Widget"));
        assert!(receipt.contains("Gadget"));
        assert!(receipt.contains("$39.97"));
        assert!(receipt.contains("39"));
    }

    #[test]
    fn test_decode_transaction_completed() {
        let bytes = vec![
            0x08, 0x90, 0x4e, // field 1: 10000
            0x12, 0x04, 0x63, 0x61, 0x72, 0x64, // field 2: "card"
            0x18, 0x64, // field 3: 100
        ];

        let event = TransactionCompleted::decode(bytes.as_slice()).unwrap();
        assert_eq!(event.final_total_cents, 10000);
        assert_eq!(event.payment_method, "card");
        assert_eq!(event.loyalty_points_earned, 100);
    }

    #[test]
    fn test_encode_receipt() {
        let receipt = Receipt {
            transaction_id: "tx123".to_string(),
            customer_id: "cust456".to_string(),
            items: vec![],
            subtotal_cents: 1000,
            discount_cents: 0,
            final_total_cents: 1000,
            payment_method: "card".to_string(),
            loyalty_points_earned: 10,
            completed_at: None,
            formatted_text: "test receipt".to_string(),
        };

        let encoded = receipt.encode_to_vec();
        assert!(!encoded.is_empty());

        // Verify round-trip
        let decoded = Receipt::decode(encoded.as_slice()).unwrap();
        assert_eq!(decoded.transaction_id, "tx123");
        assert_eq!(decoded.formatted_text, "test receipt");
    }
}
