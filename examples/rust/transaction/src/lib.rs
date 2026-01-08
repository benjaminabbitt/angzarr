//! Transaction bounded context business logic.
//!
//! Handles purchases, discounts, and transaction lifecycle.

use async_trait::async_trait;
use prost::Message;

use common::proto::{
    ApplyDiscount, CancelTransaction, CompleteTransaction, CreateTransaction, DiscountApplied,
    TransactionCancelled, TransactionCompleted, TransactionCreated, TransactionState,
};
use evented::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use evented::proto::{event_page::Sequence, CommandBook, ContextualCommand, EventBook, EventPage};

pub mod errmsg {
    pub const TRANSACTION_EXISTS: &str = "Transaction already exists";
    pub const NOT_PENDING: &str = "Transaction is not pending";
    pub const CUSTOMER_REQUIRED: &str = "customer_id is required";
    pub const ITEMS_REQUIRED: &str = "at least one item is required";
    pub const INVALID_PERCENTAGE: &str = "Percentage must be 0-100";
    pub const UNKNOWN_DISCOUNT_TYPE: &str = "Unknown discount type";
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Transaction aggregate.
pub struct TransactionLogic {
    domain: String,
}

impl TransactionLogic {
    pub const DOMAIN: &'static str = "transaction";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }

    /// Get the next sequence number from prior events.
    fn next_sequence(&self, event_book: Option<&EventBook>) -> u32 {
        event_book
            .map(|b| b.pages.len() as u32)
            .unwrap_or(0)
    }

    /// Rebuild transaction state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> TransactionState {
        let mut state = TransactionState {
            status: "new".to_string(),
            ..Default::default()
        };

        let Some(book) = event_book else {
            return state;
        };

        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.ends_with("TransactionCreated") {
                if let Ok(e) = TransactionCreated::decode(event.value.as_slice()) {
                    state.customer_id = e.customer_id;
                    state.items = e.items;
                    state.subtotal_cents = e.subtotal_cents;
                    state.status = "pending".to_string();
                }
            } else if event.type_url.ends_with("DiscountApplied") {
                if let Ok(e) = DiscountApplied::decode(event.value.as_slice()) {
                    state.discount_cents = e.discount_cents;
                    state.discount_type = e.discount_type;
                }
            } else if event.type_url.ends_with("TransactionCompleted") {
                state.status = "completed".to_string();
            } else if event.type_url.ends_with("TransactionCancelled") {
                state.status = "cancelled".to_string();
            }
        }

        state
    }

    fn handle_create_transaction(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &TransactionState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.status != "new" {
            return Err(BusinessError::Rejected(
                errmsg::TRANSACTION_EXISTS.to_string(),
            ));
        }

        let cmd = CreateTransaction::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.customer_id.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::CUSTOMER_REQUIRED.to_string(),
            ));
        }
        if cmd.items.is_empty() {
            return Err(BusinessError::Rejected(errmsg::ITEMS_REQUIRED.to_string()));
        }

        let subtotal: i32 = cmd
            .items
            .iter()
            .map(|item| item.quantity * item.unit_price_cents)
            .sum();

        let event = TransactionCreated {
            customer_id: cmd.customer_id,
            items: cmd.items,
            subtotal_cents: subtotal,
            created_at: Some(now()),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.TransactionCreated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
        })
    }

    fn handle_apply_discount(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &TransactionState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.status != "pending" {
            return Err(BusinessError::Rejected(errmsg::NOT_PENDING.to_string()));
        }

        let cmd = ApplyDiscount::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let discount_cents = match cmd.discount_type.as_str() {
            "percentage" => {
                if cmd.value < 0 || cmd.value > 100 {
                    return Err(BusinessError::Rejected(
                        errmsg::INVALID_PERCENTAGE.to_string(),
                    ));
                }
                (state.subtotal_cents * cmd.value) / 100
            }
            "fixed" => std::cmp::min(cmd.value, state.subtotal_cents),
            "coupon" => 500, // $5 off
            _ => {
                return Err(BusinessError::Rejected(format!(
                    "{}: {}",
                    errmsg::UNKNOWN_DISCOUNT_TYPE,
                    cmd.discount_type
                )));
            }
        };

        let event = DiscountApplied {
            discount_type: cmd.discount_type,
            value: cmd.value,
            discount_cents,
            coupon_code: cmd.coupon_code,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.DiscountApplied".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
        })
    }

    fn handle_complete_transaction(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &TransactionState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.status != "pending" {
            return Err(BusinessError::Rejected(errmsg::NOT_PENDING.to_string()));
        }

        let cmd = CompleteTransaction::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let final_total = std::cmp::max(0, state.subtotal_cents - state.discount_cents);
        let loyalty_points = final_total / 100; // 1 point per dollar

        let event = TransactionCompleted {
            final_total_cents: final_total,
            payment_method: cmd.payment_method,
            loyalty_points_earned: loyalty_points,
            completed_at: Some(now()),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.TransactionCompleted".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
        })
    }

    fn handle_cancel_transaction(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &TransactionState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.status != "pending" {
            return Err(BusinessError::Rejected(errmsg::NOT_PENDING.to_string()));
        }

        let cmd = CancelTransaction::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = TransactionCancelled {
            reason: cmd.reason,
            cancelled_at: Some(now()),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.TransactionCancelled".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
        })
    }
}

impl Default for TransactionLogic {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BusinessLogicClient for TransactionLogic {
    async fn handle(&self, _domain: &str, cmd: ContextualCommand) -> Result<EventBook> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);
        let next_seq = self.next_sequence(prior_events);

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

        if command_any.type_url.ends_with("CreateTransaction") {
            self.handle_create_transaction(cb, &command_any.value, &state, next_seq)
        } else if command_any.type_url.ends_with("ApplyDiscount") {
            self.handle_apply_discount(cb, &command_any.value, &state, next_seq)
        } else if command_any.type_url.ends_with("CompleteTransaction") {
            self.handle_complete_transaction(cb, &command_any.value, &state, next_seq)
        } else if command_any.type_url.ends_with("CancelTransaction") {
            self.handle_cancel_transaction(cb, &command_any.value, &state, next_seq)
        } else {
            Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            )))
        }
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
    use evented::proto::{CommandPage, Cover, Uuid as ProtoUuid};

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
        }
    }

    fn make_line_item(name: &str, quantity: i32, price_cents: i32) -> LineItem {
        LineItem {
            product_id: format!("prod-{}", name),
            name: name.to_string(),
            quantity,
            unit_price_cents: price_cents,
        }
    }

    #[tokio::test]
    async fn test_create_transaction_success() {
        let logic = TransactionLogic::new();

        let cmd = CreateTransaction {
            customer_id: "cust-123".to_string(),
            items: vec![
                make_line_item("Widget", 2, 999),
                make_line_item("Gadget", 1, 1999),
            ],
        };

        let command_book = make_command_book(
            "transaction",
            &[1; 16],
            "type.examples/examples.CreateTransaction",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let result = logic.handle("transaction", ctx).await.unwrap();
        assert_eq!(result.pages.len(), 1);

        let event =
            TransactionCreated::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.customer_id, "cust-123");
        assert_eq!(event.subtotal_cents, 2 * 999 + 1999); // 3997
    }

    #[tokio::test]
    async fn test_create_transaction_requires_customer() {
        let logic = TransactionLogic::new();

        let cmd = CreateTransaction {
            customer_id: "".to_string(),
            items: vec![make_line_item("Widget", 1, 999)],
        };

        let command_book = make_command_book(
            "transaction",
            &[1; 16],
            "type.examples/examples.CreateTransaction",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let result = logic.handle("transaction", ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("customer_id"));
    }

    #[tokio::test]
    async fn test_apply_percentage_discount() {
        let logic = TransactionLogic::new();

        // Prior events: transaction created with $100 subtotal
        let prior = EventBook {
            cover: Some(Cover {
                domain: "transaction".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1; 16],
                }),
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.TransactionCreated".to_string(),
                    value: TransactionCreated {
                        customer_id: "cust-123".to_string(),
                        items: vec![make_line_item("Widget", 10, 1000)], // $100
                        subtotal_cents: 10000,
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
                synchronous: false,
            }],
        };

        let cmd = ApplyDiscount {
            discount_type: "percentage".to_string(),
            value: 10, // 10%
            coupon_code: "".to_string(),
        };

        let command_book = make_command_book(
            "transaction",
            &[1; 16],
            "type.examples/examples.ApplyDiscount",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle("transaction", ctx).await.unwrap();
        let event =
            DiscountApplied::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.discount_cents, 1000); // 10% of $100
    }

    #[tokio::test]
    async fn test_complete_transaction_calculates_loyalty_points() {
        let logic = TransactionLogic::new();

        // Prior events: transaction with $50 subtotal
        let prior = EventBook {
            cover: Some(Cover {
                domain: "transaction".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1; 16],
                }),
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.TransactionCreated".to_string(),
                    value: TransactionCreated {
                        customer_id: "cust-123".to_string(),
                        items: vec![make_line_item("Widget", 5, 1000)], // $50
                        subtotal_cents: 5000,
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
                synchronous: false,
            }],
        };

        let cmd = CompleteTransaction {
            payment_method: "card".to_string(),
        };

        let command_book = make_command_book(
            "transaction",
            &[1; 16],
            "type.examples/examples.CompleteTransaction",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle("transaction", ctx).await.unwrap();
        let event =
            TransactionCompleted::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.final_total_cents, 5000);
        assert_eq!(event.loyalty_points_earned, 50); // 1 point per dollar
    }

    #[tokio::test]
    async fn test_cancel_transaction() {
        let logic = TransactionLogic::new();

        // Prior events: pending transaction
        let prior = EventBook {
            cover: Some(Cover {
                domain: "transaction".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1; 16],
                }),
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.TransactionCreated".to_string(),
                    value: TransactionCreated {
                        customer_id: "cust-123".to_string(),
                        items: vec![make_line_item("Widget", 1, 1000)],
                        subtotal_cents: 1000,
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
                synchronous: false,
            }],
        };

        let cmd = CancelTransaction {
            reason: "Customer changed mind".to_string(),
        };

        let command_book = make_command_book(
            "transaction",
            &[1; 16],
            "type.examples/examples.CancelTransaction",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle("transaction", ctx).await.unwrap();
        let event =
            TransactionCancelled::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.reason, "Customer changed mind");
    }

    #[tokio::test]
    async fn test_cannot_complete_cancelled_transaction() {
        let logic = TransactionLogic::new();

        // Prior events: cancelled transaction
        let prior = EventBook {
            cover: Some(Cover {
                domain: "transaction".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1; 16],
                }),
            }),
            snapshot: None,
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(0)),
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.TransactionCreated".to_string(),
                        value: TransactionCreated {
                            customer_id: "cust-123".to_string(),
                            items: vec![make_line_item("Widget", 1, 1000)],
                            subtotal_cents: 1000,
                            created_at: None,
                        }
                        .encode_to_vec(),
                    }),
                    created_at: None,
                    synchronous: false,
                },
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.TransactionCancelled".to_string(),
                        value: TransactionCancelled {
                            reason: "Cancelled".to_string(),
                            cancelled_at: None,
                        }
                        .encode_to_vec(),
                    }),
                    created_at: None,
                    synchronous: false,
                },
            ],
        };

        let cmd = CompleteTransaction {
            payment_method: "card".to_string(),
        };

        let command_book = make_command_book(
            "transaction",
            &[1; 16],
            "type.examples/examples.CompleteTransaction",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle("transaction", ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not pending"));
    }
}
