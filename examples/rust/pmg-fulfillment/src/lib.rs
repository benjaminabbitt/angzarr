//! Order Fulfillment Process Manager - fan-in across order, inventory, and fulfillment domains.
//!
//! Demonstrates the fan-in pattern: action triggers only when ALL THREE domains
//! have completed their part. A saga cannot handle this because:
//! - Each saga instance only sees one domain's event
//! - Race conditions prevent reliable "all complete" detection
//!
//! The Process Manager solves this by:
//! - Maintaining event-sourced state tracking completed prerequisites
//! - Serializing concurrent updates via aggregate sequence
//! - Using `dispatch_issued` flag for exactly-once command dispatch
//!
//! ## Subscribed Domains
//! - `order` - Listens for PaymentSubmitted
//! - `inventory` - Listens for StockReserved
//! - `fulfillment` - Listens for ItemsPacked
//!
//! ## Workflow
//! When all three events arrive (any order), emits a Ship command to fulfillment.

use prost::Message;

use angzarr::proto::{
    event_page::Sequence, CommandBook, ComponentDescriptor, Cover, EventBook, EventPage, Target,
    Uuid as ProtoUuid,
};
use common::proto::{ItemsPacked, PaymentSubmitted, Ship, StockReserved};
use common::{build_command_book, now, root_id_as_string, ProcessManagerLogic};
use uuid::Uuid;

pub const PM_NAME: &str = "pmg-fulfillment";
pub const PM_DOMAIN: &str = "pmg-fulfillment";

const ORDER_DOMAIN: &str = "order";
const INVENTORY_DOMAIN: &str = "inventory";
const FULFILLMENT_DOMAIN: &str = "fulfillment";

/// Prerequisite names tracked by the process manager.
const PREREQ_PAYMENT: &str = "payment";
const PREREQ_INVENTORY: &str = "inventory";
const PREREQ_FULFILLMENT: &str = "fulfillment";

/// All prerequisites that must complete before dispatch.
const ALL_PREREQUISITES: &[&str] = &[PREREQ_PAYMENT, PREREQ_INVENTORY, PREREQ_FULFILLMENT];

/// Order Fulfillment Process Manager.
///
/// Tracks three prerequisites across domains. When all are met, issues Ship command.
pub struct OrderFulfillmentProcess;

impl OrderFulfillmentProcess {
    pub fn new() -> Self {
        Self
    }

    /// Classify a trigger event into a prerequisite name.
    fn classify_event(event: &prost_types::Any) -> Option<&'static str> {
        if event.type_url.ends_with("PaymentSubmitted") {
            PaymentSubmitted::decode(event.value.as_slice())
                .ok()
                .map(|_| PREREQ_PAYMENT)
        } else if event.type_url.ends_with("StockReserved") {
            StockReserved::decode(event.value.as_slice())
                .ok()
                .map(|_| PREREQ_INVENTORY)
        } else if event.type_url.ends_with("ItemsPacked") {
            ItemsPacked::decode(event.value.as_slice())
                .ok()
                .map(|_| PREREQ_FULFILLMENT)
        } else {
            None
        }
    }

    /// Extract completed prerequisites from process manager state events.
    fn extract_completed(process_state: Option<&EventBook>) -> Vec<String> {
        let Some(state) = process_state else {
            return vec![];
        };

        let mut completed = vec![];
        for page in &state.pages {
            if let Some(event) = &page.event {
                if event.type_url.ends_with("PrerequisiteCompleted") {
                    if let Ok(evt) = PrerequisiteCompleted::decode(event.value.as_slice()) {
                        if !completed.contains(&evt.prerequisite) {
                            completed.push(evt.prerequisite);
                        }
                    }
                } else if event.type_url.ends_with("DispatchIssued") {
                    // Already dispatched - return special marker
                    completed.push("__dispatched__".to_string());
                }
            }
        }
        completed
    }

    /// Check if all prerequisites are met.
    fn all_complete(completed: &[String]) -> bool {
        ALL_PREREQUISITES
            .iter()
            .all(|p| completed.iter().any(|c| c == p))
    }

    /// Check if dispatch was already issued (idempotency).
    fn already_dispatched(completed: &[String]) -> bool {
        completed.iter().any(|c| c == "__dispatched__")
    }
}

impl Default for OrderFulfillmentProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManagerLogic for OrderFulfillmentProcess {
    fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: PM_NAME.to_string(),
            component_type: "process_manager".to_string(),
            inputs: vec![
                Target {
                    domain: ORDER_DOMAIN.to_string(),
                    types: vec!["PaymentSubmitted".to_string()],
                },
                Target {
                    domain: INVENTORY_DOMAIN.to_string(),
                    types: vec!["StockReserved".to_string()],
                },
                Target {
                    domain: FULFILLMENT_DOMAIN.to_string(),
                    types: vec!["ItemsPacked".to_string()],
                },
            ],
            outputs: vec![Target {
                domain: FULFILLMENT_DOMAIN.to_string(),
                types: vec!["Ship".to_string()],
            }],
        }
    }

    fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
        // No additional destinations needed - PM only uses its own state
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

        // Get current completed prerequisites from PM state
        let mut completed = Self::extract_completed(process_state);

        // Already dispatched - idempotent no-op
        if Self::already_dispatched(&completed) {
            return (vec![], None);
        }

        // Classify the trigger event
        let mut new_prerequisite = None;
        for page in &trigger.pages {
            if let Some(event) = &page.event {
                if let Some(prereq) = Self::classify_event(event) {
                    if !completed.iter().any(|c| c == prereq) {
                        completed.push(prereq.to_string());
                        new_prerequisite = Some(prereq);
                    }
                }
            }
        }

        // No new prerequisite from this event
        let Some(prereq) = new_prerequisite else {
            return (vec![], None);
        };

        // Build PM events
        // Derive a deterministic root from correlation_id so all trigger events
        // (from different domains with different roots) map to the same PM aggregate.
        let pm_root = Some(ProtoUuid {
            value: Uuid::new_v5(&Uuid::NAMESPACE_OID, correlation_id.as_bytes())
                .as_bytes()
                .to_vec(),
        });
        let next_seq = process_state
            .and_then(|s| s.pages.last())
            .and_then(|p| match &p.sequence {
                Some(Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        let mut pm_events = vec![];
        let mut commands = vec![];

        // Record prerequisite completion
        let prereq_event = PrerequisiteCompleted {
            prerequisite: prereq.to_string(),
            completed: completed.clone(),
            remaining: ALL_PREREQUISITES
                .iter()
                .filter(|p| !completed.iter().any(|c| c == **p))
                .map(|p| p.to_string())
                .collect(),
        };

        pm_events.push(EventPage {
            sequence: Some(Sequence::Num(next_seq)),
            created_at: Some(now()),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.PrerequisiteCompleted".to_string(),
                value: prereq_event.encode_to_vec(),
            }),
        });

        // Check if all prerequisites met
        if Self::all_complete(&completed) {
            // Record dispatch
            let dispatch_event = DispatchIssued {
                completed: completed.clone(),
            };

            pm_events.push(EventPage {
                sequence: Some(Sequence::Num(next_seq + 1)),
                created_at: Some(now()),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.DispatchIssued".to_string(),
                    value: dispatch_event.encode_to_vec(),
                }),
            });

            // Emit Ship command to fulfillment domain
            let order_id = root_id_as_string(trigger.cover.as_ref().and_then(|c| c.root.as_ref()));

            let ship_cmd = Ship {
                carrier: format!("auto-{order_id}"),
                tracking_number: String::new(),
            };

            commands.push(build_command_book(
                FULFILLMENT_DOMAIN,
                trigger.cover.as_ref().and_then(|c| c.root.clone()),
                correlation_id,
                "type.examples/examples.Ship",
                &ship_cmd,
            ));
        }

        let pm_event_book = if pm_events.is_empty() {
            None
        } else {
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

        (commands, pm_event_book)
    }
}

// Standalone runtime support — delegates to ProcessManagerLogic methods.
#[cfg(feature = "standalone")]
impl angzarr::standalone::ProcessManagerHandler for OrderFulfillmentProcess {
    fn descriptor(&self) -> ComponentDescriptor {
        ProcessManagerLogic::descriptor(self)
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

// Process Manager internal events (not shared with other domains)

/// A prerequisite was completed in the workflow.
#[derive(Clone, PartialEq, prost::Message)]
pub struct PrerequisiteCompleted {
    #[prost(string, tag = "1")]
    pub prerequisite: String,
    #[prost(string, repeated, tag = "2")]
    pub completed: Vec<String>,
    #[prost(string, repeated, tag = "3")]
    pub remaining: Vec<String>,
}

/// All prerequisites met, dispatch command issued.
#[derive(Clone, PartialEq, prost::Message)]
pub struct DispatchIssued {
    #[prost(string, repeated, tag = "1")]
    pub completed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event_book(domain: &str, event: prost_types::Any, correlation_id: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v5(&Uuid::NAMESPACE_OID, b"order-123")
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

    fn payment_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.PaymentSubmitted".to_string(),
            value: PaymentSubmitted {
                payment_method: "card".to_string(),
                amount_cents: 5000,
                submitted_at: None,
            }
            .encode_to_vec(),
        }
    }

    fn stock_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.StockReserved".to_string(),
            value: StockReserved {
                quantity: 1,
                order_id: "order-123".to_string(),
                new_available: 9,
                reserved_at: None,
                new_reserved: 1,
                new_on_hand: 10,
            }
            .encode_to_vec(),
        }
    }

    fn shipment_event() -> prost_types::Any {
        prost_types::Any {
            type_url: "type.examples/examples.ItemsPacked".to_string(),
            value: ItemsPacked {
                packer_id: "packer-1".to_string(),
                packed_at: None,
            }
            .encode_to_vec(),
        }
    }

    #[test]
    fn test_first_event_no_dispatch() {
        let pm = OrderFulfillmentProcess::new();
        let trigger = make_event_book("order", payment_event(), "corr-1");

        let (commands, pm_events) = pm.handle(&trigger, None, &[]);

        assert!(commands.is_empty(), "Should not dispatch on first event");
        assert!(pm_events.is_some(), "Should produce PM events");

        let events = pm_events.unwrap();
        assert_eq!(events.pages.len(), 1, "One prerequisite completed");
    }

    #[test]
    fn test_second_event_no_dispatch() {
        let pm = OrderFulfillmentProcess::new();

        // First event produced PM state
        let trigger1 = make_event_book("order", payment_event(), "corr-1");
        let (_, pm_state1) = pm.handle(&trigger1, None, &[]);

        // Second event
        let trigger2 = make_event_book("inventory", stock_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger2, pm_state1.as_ref(), &[]);

        assert!(commands.is_empty(), "Should not dispatch on second event");
        assert!(pm_events.is_some());
    }

    #[test]
    fn test_third_event_triggers_dispatch() {
        let pm = OrderFulfillmentProcess::new();

        // First event
        let trigger1 = make_event_book("order", payment_event(), "corr-1");
        let (_, pm_state1) = pm.handle(&trigger1, None, &[]);

        // Second event
        let trigger2 = make_event_book("inventory", stock_event(), "corr-1");
        let (_, pm_state2) = pm.handle(&trigger2, pm_state1.as_ref(), &[]);

        // Merge state: combine pm_state1 + pm_state2 pages
        let merged_state = merge_pm_states(pm_state1, pm_state2);

        // Third event - should trigger dispatch
        let trigger3 = make_event_book("fulfillment", shipment_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger3, merged_state.as_ref(), &[]);

        assert_eq!(commands.len(), 1, "Should dispatch Ship command");
        assert_eq!(
            commands[0].cover.as_ref().unwrap().domain,
            FULFILLMENT_DOMAIN
        );

        let events = pm_events.unwrap();
        assert_eq!(
            events.pages.len(),
            2,
            "PrerequisiteCompleted + DispatchIssued"
        );
    }

    #[test]
    fn test_idempotent_after_dispatch() {
        let pm = OrderFulfillmentProcess::new();

        // Build state that includes DispatchIssued
        let dispatched_state = EventBook {
            cover: Some(Cover {
                domain: PM_DOMAIN.to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v5(&Uuid::NAMESPACE_OID, b"order-123")
                        .as_bytes()
                        .to_vec(),
                }),
                correlation_id: "corr-1".to_string(),
                edition: None,
            }),
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(0)),
                    created_at: None,
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.PrerequisiteCompleted".to_string(),
                        value: PrerequisiteCompleted {
                            prerequisite: PREREQ_PAYMENT.to_string(),
                            completed: vec![PREREQ_PAYMENT.to_string()],
                            remaining: vec![
                                PREREQ_INVENTORY.to_string(),
                                PREREQ_FULFILLMENT.to_string(),
                            ],
                        }
                        .encode_to_vec(),
                    }),
                },
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    created_at: None,
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.DispatchIssued".to_string(),
                        value: DispatchIssued {
                            completed: vec![
                                PREREQ_PAYMENT.to_string(),
                                PREREQ_INVENTORY.to_string(),
                                PREREQ_FULFILLMENT.to_string(),
                            ],
                        }
                        .encode_to_vec(),
                    }),
                },
            ],
            snapshot: None,
            snapshot_state: None,
        };

        // Another event arrives - should be no-op
        let trigger = make_event_book("order", payment_event(), "corr-1");
        let (commands, pm_events) = pm.handle(&trigger, Some(&dispatched_state), &[]);

        assert!(commands.is_empty(), "Should not dispatch again");
        assert!(pm_events.is_none(), "Should not produce events");
    }

    #[test]
    fn test_no_correlation_id_skips() {
        let pm = OrderFulfillmentProcess::new();
        let trigger = make_event_book("order", payment_event(), "");

        let (commands, pm_events) = pm.handle(&trigger, None, &[]);

        assert!(commands.is_empty());
        assert!(pm_events.is_none());
    }

    #[test]
    fn test_descriptor() {
        let pm = OrderFulfillmentProcess::new();
        let desc = pm.descriptor();

        assert_eq!(desc.name, PM_NAME);
        assert_eq!(desc.component_type, "process_manager");
        assert_eq!(desc.inputs.len(), 3);
        assert_eq!(desc.inputs[0].domain, ORDER_DOMAIN);
        assert_eq!(desc.inputs[1].domain, INVENTORY_DOMAIN);
        assert_eq!(desc.inputs[2].domain, FULFILLMENT_DOMAIN);
        assert_eq!(desc.outputs.len(), 1);
        assert_eq!(desc.outputs[0].domain, FULFILLMENT_DOMAIN);
        assert_eq!(desc.outputs[0].types, vec!["Ship"]);
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
}
