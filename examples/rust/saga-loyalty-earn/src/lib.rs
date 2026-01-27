//! Loyalty Earn Saga - awards loyalty points when orders complete.
//!
//! Listens to OrderCompleted events and generates AddLoyaltyPoints commands.

use prost::Message;

use angzarr::proto::{CommandBook, CommandPage, Cover, EventBook, Uuid as ProtoUuid};
use common::proto::{AddLoyaltyPoints, OrderCompleted};
use common::SagaLogic;

pub const SAGA_NAME: &str = "loyalty-earn";
pub const SOURCE_DOMAIN: &str = "order";
pub const TARGET_DOMAIN: &str = "customer";

/// Loyalty Earn Saga implementation.
pub struct LoyaltyEarnSaga;

impl LoyaltyEarnSaga {
    pub fn new() -> Self {
        Self
    }

    /// Process a single event page and generate a command if applicable.
    fn process_event(
        &self,
        event: &prost_types::Any,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Option<CommandBook> {
        // Only process OrderCompleted events
        if !event.type_url.ends_with("OrderCompleted") {
            return None;
        }

        let completed = OrderCompleted::decode(event.value.as_slice()).ok()?;

        // Don't generate command for zero points
        if completed.loyalty_points_earned <= 0 {
            return None;
        }

        // Generate AddLoyaltyPoints command targeting customer domain
        let cmd = AddLoyaltyPoints {
            points: completed.loyalty_points_earned,
            reason: format!("order:{}", correlation_id),
        };

        let cmd_any = prost_types::Any {
            type_url: "type.examples/examples.AddLoyaltyPoints".to_string(),
            value: cmd.encode_to_vec(),
        };

        Some(CommandBook {
            cover: Some(Cover {
                domain: TARGET_DOMAIN.to_string(),
                root: source_root.cloned(),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(cmd_any),
            }],
            correlation_id: correlation_id.to_string(),
            ..Default::default()
        })
    }

    /// Handle an event book, producing commands for any relevant events.
    pub fn handle(&self, book: &EventBook) -> Vec<CommandBook> {
        let source_root = book.cover.as_ref().and_then(|c| c.root.as_ref());
        let correlation_id = &book.correlation_id;

        book.pages
            .iter()
            .filter_map(|page| {
                page.event
                    .as_ref()
                    .and_then(|e| self.process_event(e, source_root, correlation_id))
            })
            .collect()
    }
}

impl Default for LoyaltyEarnSaga {
    fn default() -> Self {
        Self::new()
    }
}

impl SagaLogic for LoyaltyEarnSaga {
    /// This saga doesn't need destination state - just produces commands from source events.
    fn prepare(&self, _source: &EventBook) -> Vec<Cover> {
        vec![]
    }

    fn execute(&self, source: &EventBook, _destinations: &[EventBook]) -> Vec<CommandBook> {
        self.handle(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::EventPage;

    #[test]
    fn test_process_order_completed() {
        let saga = LoyaltyEarnSaga::new();

        let event = OrderCompleted {
            final_total_cents: 5000,
            payment_method: "card".to_string(),
            payment_reference: "PAY-001".to_string(),
            loyalty_points_earned: 50,
            completed_at: None,
        };

        let event_any = prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: event.encode_to_vec(),
        };

        let root = ProtoUuid {
            value: vec![1, 2, 3, 4],
        };

        let book = EventBook {
            cover: Some(Cover {
                domain: SOURCE_DOMAIN.to_string(),
                root: Some(root),
            }),
            snapshot: None,
            pages: vec![EventPage {
                created_at: None,
                sequence: Some(angzarr::proto::event_page::Sequence::Num(1)),
                event: Some(event_any),
            }],
            correlation_id: "CORR-001".to_string(),
            snapshot_state: None,
        };

        let commands = saga.handle(&book);
        assert_eq!(commands.len(), 1);

        let cmd = &commands[0];
        assert_eq!(cmd.cover.as_ref().unwrap().domain, TARGET_DOMAIN);
        assert_eq!(cmd.correlation_id, "CORR-001");

        let cmd_any = cmd.pages[0].command.as_ref().unwrap();
        let add_points = AddLoyaltyPoints::decode(cmd_any.value.as_slice()).expect("Should decode");
        assert_eq!(add_points.points, 50);
        assert!(add_points.reason.contains("order"));
    }

    #[test]
    fn test_ignore_zero_points() {
        let saga = LoyaltyEarnSaga::new();

        let event = OrderCompleted {
            final_total_cents: 50, // Only 50 cents = 0 points
            payment_method: "card".to_string(),
            payment_reference: "PAY-002".to_string(),
            loyalty_points_earned: 0,
            completed_at: None,
        };

        let event_any = prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: event.encode_to_vec(),
        };

        let book = EventBook {
            cover: Some(Cover {
                domain: SOURCE_DOMAIN.to_string(),
                root: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                created_at: None,
                sequence: Some(angzarr::proto::event_page::Sequence::Num(1)),
                event: Some(event_any),
            }],
            correlation_id: "CORR-002".to_string(),
            snapshot_state: None,
        };

        let commands = saga.handle(&book);
        assert!(commands.is_empty());
    }
}
