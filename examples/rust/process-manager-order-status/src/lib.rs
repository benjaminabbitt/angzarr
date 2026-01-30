//! Order Status Process Manager - cross-domain order lifecycle observer.
//!
//! Subscribes to events across order, inventory, and fulfillment domains
//! to maintain a unified status view of each order's lifecycle.
//!
//! This PM is purely observational — it records status transitions as
//! internal events but does NOT issue commands. It demonstrates the
//! multi-domain observation pattern for process managers.
//!
//! ## Subscribed Domains
//! - `order` — OrderCreated, PaymentSubmitted, OrderCompleted, OrderCancelled
//! - `inventory` — StockReserved
//! - `fulfillment` — ShipmentCreated
//!
//! ## Status State Machine
//! ```text
//! (none)             → OrderCreated      → "created"
//! "created"          → PaymentSubmitted  → "payment_received"
//! "created"          → StockReserved     → "stock_reserved"
//! "payment_received" → StockReserved     → "ready"
//! "stock_reserved"   → PaymentSubmitted  → "ready"
//! "ready"            → OrderCompleted    → "completed"
//! "payment_received" → OrderCompleted    → "completed"
//! "created"          → OrderCompleted    → "completed"
//! "completed"        → ShipmentCreated   → "shipping"
//! any non-terminal   → OrderCancelled    → "cancelled"
//! ```

use prost::Message;

use angzarr::proto::{
    event_page::Sequence, CommandBook, Cover, EventBook, EventPage, Subscription, Uuid as ProtoUuid,
};
use common::{now, ProcessManagerLogic};
use uuid::Uuid;

pub const PM_NAME: &str = "order-status";
pub const PM_DOMAIN: &str = "order-status";

const ORDER_DOMAIN: &str = "order";
const INVENTORY_DOMAIN: &str = "inventory";
const FULFILLMENT_DOMAIN: &str = "fulfillment";

// Status values
const STATUS_CREATED: &str = "created";
const STATUS_PAYMENT_RECEIVED: &str = "payment_received";
const STATUS_STOCK_RESERVED: &str = "stock_reserved";
const STATUS_READY: &str = "ready";
const STATUS_COMPLETED: &str = "completed";
const STATUS_SHIPPING: &str = "shipping";
const STATUS_CANCELLED: &str = "cancelled";

/// Terminal statuses — no further transitions allowed.
const TERMINAL_STATUSES: &[&str] = &[STATUS_SHIPPING, STATUS_CANCELLED];

/// Order Status Process Manager.
///
/// Observes order lifecycle events across domains and records status transitions.
pub struct OrderStatusProcess;

impl OrderStatusProcess {
    pub fn new() -> Self {
        Self
    }

    /// Classify a trigger event into a target status transition.
    ///
    /// Returns `(trigger_event_name, target_status_fn)` where `target_status_fn`
    /// maps current_status → Option<new_status>.
    fn classify_trigger(event: &prost_types::Any) -> Option<(&'static str, &'static str)> {
        let type_url = &event.type_url;
        if type_url.ends_with("OrderCreated") {
            Some(("OrderCreated", ""))
        } else if type_url.ends_with("PaymentSubmitted") {
            Some(("PaymentSubmitted", ""))
        } else if type_url.ends_with("StockReserved") {
            Some(("StockReserved", ""))
        } else if type_url.ends_with("OrderCompleted") {
            Some(("OrderCompleted", ""))
        } else if type_url.ends_with("OrderCancelled") {
            Some(("OrderCancelled", ""))
        } else if type_url.ends_with("ShipmentCreated") {
            Some(("ShipmentCreated", ""))
        } else {
            None
        }
    }

    /// Compute the next status given the current status and trigger event.
    ///
    /// Returns None if the transition is invalid (terminal state, duplicate, etc.).
    fn compute_transition(current_status: &str, trigger_event: &str) -> Option<&'static str> {
        // Terminal states reject all transitions
        if TERMINAL_STATUSES.contains(&current_status) {
            return None;
        }

        match (current_status, trigger_event) {
            // Initial creation
            ("", "OrderCreated") => Some(STATUS_CREATED),

            // From "created"
            (STATUS_CREATED, "PaymentSubmitted") => Some(STATUS_PAYMENT_RECEIVED),
            (STATUS_CREATED, "StockReserved") => Some(STATUS_STOCK_RESERVED),
            (STATUS_CREATED, "OrderCompleted") => Some(STATUS_COMPLETED),
            (STATUS_CREATED, "OrderCancelled") => Some(STATUS_CANCELLED),

            // From "payment_received"
            (STATUS_PAYMENT_RECEIVED, "StockReserved") => Some(STATUS_READY),
            (STATUS_PAYMENT_RECEIVED, "OrderCompleted") => Some(STATUS_COMPLETED),
            (STATUS_PAYMENT_RECEIVED, "OrderCancelled") => Some(STATUS_CANCELLED),

            // From "stock_reserved"
            (STATUS_STOCK_RESERVED, "PaymentSubmitted") => Some(STATUS_READY),
            (STATUS_STOCK_RESERVED, "OrderCompleted") => Some(STATUS_COMPLETED),
            (STATUS_STOCK_RESERVED, "OrderCancelled") => Some(STATUS_CANCELLED),

            // From "ready"
            (STATUS_READY, "OrderCompleted") => Some(STATUS_COMPLETED),
            (STATUS_READY, "OrderCancelled") => Some(STATUS_CANCELLED),

            // From "completed" — only ShipmentCreated
            (STATUS_COMPLETED, "ShipmentCreated") => Some(STATUS_SHIPPING),

            // All other transitions are no-ops (duplicate or invalid)
            _ => None,
        }
    }

    /// Extract current status from PM state by replaying OrderStatusChanged events.
    fn extract_current_status(process_state: Option<&EventBook>) -> String {
        let Some(state) = process_state else {
            return String::new();
        };

        let mut current = String::new();
        for page in &state.pages {
            if let Some(event) = &page.event {
                if event.type_url.ends_with("OrderStatusChanged") {
                    if let Ok(evt) = OrderStatusChanged::decode(event.value.as_slice()) {
                        current = evt.to_status;
                    }
                }
            }
        }
        current
    }

    /// Extract the trigger domain from the EventBook cover.
    fn trigger_domain(trigger: &EventBook) -> &str {
        trigger
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("")
    }
}

impl Default for OrderStatusProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManagerLogic for OrderStatusProcess {
    fn subscriptions(&self) -> Vec<Subscription> {
        vec![
            Subscription {
                domain: ORDER_DOMAIN.to_string(),
                event_types: vec![
                    "OrderCreated".to_string(),
                    "PaymentSubmitted".to_string(),
                    "OrderCompleted".to_string(),
                    "OrderCancelled".to_string(),
                ],
            },
            Subscription {
                domain: INVENTORY_DOMAIN.to_string(),
                event_types: vec!["StockReserved".to_string()],
            },
            Subscription {
                domain: FULFILLMENT_DOMAIN.to_string(),
                event_types: vec!["ShipmentCreated".to_string()],
            },
        ]
    }

    fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
        // No additional destinations needed — PM only uses its own state
        vec![]
    }

    fn handle(
        &self,
        trigger: &EventBook,
        process_state: Option<&EventBook>,
        _destinations: &[EventBook],
    ) -> (Vec<CommandBook>, Option<EventBook>) {
        let correlation_id = trigger
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or_default();

        if correlation_id.is_empty() {
            return (vec![], None);
        }

        let current_status = Self::extract_current_status(process_state);
        let domain = Self::trigger_domain(trigger);

        // Process each event page in the trigger
        let mut new_status = current_status.clone();
        let mut pm_events = vec![];
        let next_seq = process_state
            .and_then(|s| s.pages.last())
            .and_then(|p| match &p.sequence {
                Some(Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        for page in &trigger.pages {
            if let Some(event) = &page.event {
                if let Some((trigger_event, _)) = Self::classify_trigger(event) {
                    if let Some(target) = Self::compute_transition(&new_status, trigger_event) {
                        let status_changed = OrderStatusChanged {
                            from_status: new_status.clone(),
                            to_status: target.to_string(),
                            trigger_event: trigger_event.to_string(),
                            trigger_domain: domain.to_string(),
                        };

                        pm_events.push(EventPage {
                            sequence: Some(Sequence::Num(next_seq + pm_events.len() as u32)),
                            created_at: Some(now()),
                            event: Some(prost_types::Any {
                                type_url: "type.examples/examples.OrderStatusChanged".to_string(),
                                value: status_changed.encode_to_vec(),
                            }),
                        });

                        new_status = target.to_string();
                    }
                }
            }
        }

        // No commands issued — this PM is purely observational
        let pm_event_book = if pm_events.is_empty() {
            None
        } else {
            let pm_root = Some(ProtoUuid {
                value: Uuid::new_v5(&Uuid::NAMESPACE_OID, correlation_id.as_bytes())
                    .as_bytes()
                    .to_vec(),
            });

            Some(EventBook {
                cover: Some(Cover {
                    domain: PM_DOMAIN.to_string(),
                    root: pm_root,
                    correlation_id: correlation_id.to_string(),
                    edition: None,
                }),
                pages: pm_events,
                snapshot: None,
                snapshot_state: None,
            })
        };

        (vec![], pm_event_book)
    }
}

// Standalone runtime support — delegates to ProcessManagerLogic methods.
#[cfg(feature = "standalone")]
impl angzarr::standalone::ProcessManagerHandler for OrderStatusProcess {
    fn subscriptions(&self) -> Vec<Subscription> {
        ProcessManagerLogic::subscriptions(self)
    }

    fn prepare(&self, trigger: &EventBook, process_state: Option<&EventBook>) -> Vec<Cover> {
        ProcessManagerLogic::prepare(self, trigger, process_state)
    }

    fn handle(
        &self,
        trigger: &EventBook,
        process_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> (Vec<CommandBook>, Option<EventBook>) {
        ProcessManagerLogic::handle(self, trigger, process_state, destinations)
    }
}

/// Records a status transition in the order lifecycle.
#[derive(Clone, PartialEq, prost::Message)]
pub struct OrderStatusChanged {
    #[prost(string, tag = "1")]
    pub from_status: String,
    #[prost(string, tag = "2")]
    pub to_status: String,
    #[prost(string, tag = "3")]
    pub trigger_event: String,
    #[prost(string, tag = "4")]
    pub trigger_domain: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::proto;

    fn make_event_book(domain: &str, event: prost_types::Any, correlation_id: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v5(&Uuid::NAMESPACE_OID, b"order-test")
                        .as_bytes()
                        .to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                created_at: None,
                event: Some(event),
            }],
            snapshot: None,
            snapshot_state: None,
        }
    }

    fn order_created_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.OrderCreated".to_string(),
            value: proto::OrderCreated {
                customer_id: "cust-1".to_string(),
                items: vec![],
                subtotal_cents: 1000,
                created_at: None,
                customer_root: vec![],
                cart_root: vec![],
            }
            .encode_to_vec(),
        }
    }

    fn payment_submitted_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.PaymentSubmitted".to_string(),
            value: proto::PaymentSubmitted {
                payment_method: "card".to_string(),
                amount_cents: 1000,
                submitted_at: None,
            }
            .encode_to_vec(),
        }
    }

    fn stock_reserved_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.StockReserved".to_string(),
            value: proto::StockReserved {
                quantity: 1,
                order_id: "order-test".to_string(),
                new_available: 99,
                reserved_at: None,
                new_reserved: 1,
                new_on_hand: 100,
            }
            .encode_to_vec(),
        }
    }

    fn order_completed_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: proto::OrderCompleted {
                final_total_cents: 1000,
                payment_method: "card".to_string(),
                payment_reference: "ref-1".to_string(),
                loyalty_points_earned: 10,
                completed_at: None,
                customer_root: vec![],
                cart_root: vec![],
                items: vec![],
            }
            .encode_to_vec(),
        }
    }

    fn order_cancelled_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.OrderCancelled".to_string(),
            value: proto::OrderCancelled {
                reason: "customer request".to_string(),
                cancelled_at: None,
                loyalty_points_used: 0,
                customer_root: vec![],
                items: vec![],
                cart_root: vec![],
            }
            .encode_to_vec(),
        }
    }

    fn shipment_created_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.ShipmentCreated".to_string(),
            value: proto::ShipmentCreated {
                order_id: "order-test".to_string(),
                status: "created".to_string(),
                created_at: None,
            }
            .encode_to_vec(),
        }
    }

    fn extract_to_status(pm_events: &Option<EventBook>) -> String {
        let events = pm_events.as_ref().expect("Expected PM events");
        let last = events.pages.last().expect("Expected at least one event");
        let event = last.event.as_ref().expect("Expected event payload");
        let changed = OrderStatusChanged::decode(event.value.as_slice())
            .expect("Failed to decode OrderStatusChanged");
        changed.to_status
    }

    fn extract_from_status(pm_events: &Option<EventBook>) -> String {
        let events = pm_events.as_ref().expect("Expected PM events");
        let last = events.pages.last().expect("Expected at least one event");
        let event = last.event.as_ref().expect("Expected event payload");
        let changed = OrderStatusChanged::decode(event.value.as_slice())
            .expect("Failed to decode OrderStatusChanged");
        changed.from_status
    }

    /// Merge two PM state EventBooks (simulating persisted state accumulation).
    fn merge_pm_states(state1: Option<EventBook>, state2: Option<EventBook>) -> Option<EventBook> {
        match (state1, state2) {
            (Some(mut s1), Some(s2)) => {
                s1.pages.extend(s2.pages);
                Some(s1)
            }
            (s1, None) => s1,
            (None, s2) => s2,
        }
    }

    #[test]
    fn test_order_created_transitions_to_created() {
        let pm = OrderStatusProcess::new();
        let trigger = make_event_book("order", order_created_event(), "corr-1");

        let (commands, pm_events) = pm.handle(&trigger, None, &[]);

        assert!(commands.is_empty(), "Observer PM should not issue commands");
        assert_eq!(extract_to_status(&pm_events), STATUS_CREATED);
        assert_eq!(extract_from_status(&pm_events), "");
    }

    #[test]
    fn test_payment_submitted_transitions_to_payment_received() {
        let pm = OrderStatusProcess::new();

        // First: OrderCreated
        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        // Second: PaymentSubmitted
        let trigger2 = make_event_book("order", payment_submitted_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger2, state1.as_ref(), &[]);

        assert!(commands.is_empty());
        assert_eq!(extract_to_status(&pm_events), STATUS_PAYMENT_RECEIVED);
        assert_eq!(extract_from_status(&pm_events), STATUS_CREATED);
    }

    #[test]
    fn test_stock_reserved_transitions_to_stock_reserved() {
        let pm = OrderStatusProcess::new();

        // First: OrderCreated
        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        // Second: StockReserved
        let trigger2 = make_event_book("inventory", stock_reserved_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger2, state1.as_ref(), &[]);

        assert!(commands.is_empty());
        assert_eq!(extract_to_status(&pm_events), STATUS_STOCK_RESERVED);
    }

    #[test]
    fn test_payment_then_stock_transitions_to_ready() {
        let pm = OrderStatusProcess::new();

        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        let trigger2 = make_event_book("order", payment_submitted_event(), "corr-1");
        let (_, state2) = pm.handle(&trigger2, state1.as_ref(), &[]);
        let merged = merge_pm_states(state1, state2);

        let trigger3 = make_event_book("inventory", stock_reserved_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger3, merged.as_ref(), &[]);

        assert!(commands.is_empty());
        assert_eq!(extract_to_status(&pm_events), STATUS_READY);
    }

    #[test]
    fn test_stock_then_payment_transitions_to_ready() {
        let pm = OrderStatusProcess::new();

        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        let trigger2 = make_event_book("inventory", stock_reserved_event(), "corr-1");
        let (_, state2) = pm.handle(&trigger2, state1.as_ref(), &[]);
        let merged = merge_pm_states(state1, state2);

        let trigger3 = make_event_book("order", payment_submitted_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger3, merged.as_ref(), &[]);

        assert!(commands.is_empty());
        assert_eq!(extract_to_status(&pm_events), STATUS_READY);
    }

    #[test]
    fn test_order_completed_transitions_to_completed() {
        let pm = OrderStatusProcess::new();

        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        let trigger2 = make_event_book("order", order_completed_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger2, state1.as_ref(), &[]);

        assert!(commands.is_empty());
        assert_eq!(extract_to_status(&pm_events), STATUS_COMPLETED);
    }

    #[test]
    fn test_shipment_created_transitions_to_shipping() {
        let pm = OrderStatusProcess::new();

        // Build state: created → completed
        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        let trigger2 = make_event_book("order", order_completed_event(), "corr-1");
        let (_, state2) = pm.handle(&trigger2, state1.as_ref(), &[]);
        let merged = merge_pm_states(state1, state2);

        // ShipmentCreated
        let trigger3 = make_event_book("fulfillment", shipment_created_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger3, merged.as_ref(), &[]);

        assert!(commands.is_empty());
        assert_eq!(extract_to_status(&pm_events), STATUS_SHIPPING);
    }

    #[test]
    fn test_order_cancelled_is_terminal() {
        let pm = OrderStatusProcess::new();

        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        let trigger2 = make_event_book("order", order_cancelled_event(), "corr-1");
        let (_, state2) = pm.handle(&trigger2, state1.as_ref(), &[]);
        let merged = merge_pm_states(state1, state2);

        // After cancellation, PaymentSubmitted should be no-op
        let trigger3 = make_event_book("order", payment_submitted_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger3, merged.as_ref(), &[]);

        assert!(commands.is_empty());
        assert!(pm_events.is_none(), "Terminal state should produce no events");
    }

    #[test]
    fn test_duplicate_event_is_noop() {
        let pm = OrderStatusProcess::new();

        let trigger1 = make_event_book("order", order_created_event(), "corr-1");
        let (_, state1) = pm.handle(&trigger1, None, &[]);

        // Duplicate OrderCreated — already in "created" state
        let trigger2 = make_event_book("order", order_created_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger2, state1.as_ref(), &[]);

        assert!(commands.is_empty());
        assert!(
            pm_events.is_none(),
            "Duplicate event should produce no events"
        );
    }

    #[test]
    fn test_no_correlation_skips() {
        let pm = OrderStatusProcess::new();
        let trigger = make_event_book("order", order_created_event(), "");

        let (commands, pm_events) = pm.handle(&trigger, None, &[]);

        assert!(commands.is_empty());
        assert!(pm_events.is_none());
    }

    #[test]
    fn test_subscriptions() {
        let pm = OrderStatusProcess::new();
        let subs = pm.subscriptions();

        assert_eq!(subs.len(), 3);
        assert_eq!(subs[0].domain, ORDER_DOMAIN);
        assert_eq!(subs[0].event_types.len(), 4);
        assert_eq!(subs[1].domain, INVENTORY_DOMAIN);
        assert_eq!(subs[1].event_types, vec!["StockReserved"]);
        assert_eq!(subs[2].domain, FULFILLMENT_DOMAIN);
        assert_eq!(subs[2].event_types, vec!["ShipmentCreated"]);
    }
}
