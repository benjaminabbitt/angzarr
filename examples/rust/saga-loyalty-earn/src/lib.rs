//! Loyalty Earn Saga - awards loyalty points and commits inventory when orders complete.
//!
//! Listens to OrderCompleted events and generates:
//! - AddLoyaltyPoints command (to customer)
//! - CommitReservation commands (to inventory, for each item)

use angzarr::proto::{CommandBook, EventBook, Uuid as ProtoUuid};
use common::proto::{AddLoyaltyPoints, CommitReservation, OrderCompleted};
use common::{build_command_book, decode_event, process_event_pages, root_id_as_string};

pub const SAGA_NAME: &str = "loyalty-earn";
pub const SOURCE_DOMAIN: &str = "order";
pub const CUSTOMER_DOMAIN: &str = "customer";
pub const INVENTORY_DOMAIN: &str = "inventory";

/// Loyalty Earn Saga implementation.
pub struct LoyaltyEarnSaga;
common::define_saga!(LoyaltyEarnSaga);

impl LoyaltyEarnSaga {
    /// Process a single event page and generate commands if applicable.
    fn process_event(
        &self,
        event: &prost_types::Any,
        _source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        // Only process OrderCompleted events
        let Some(completed) = decode_event::<OrderCompleted>(event, "OrderCompleted") else {
            return vec![];
        };

        let mut commands = Vec::new();

        // Generate AddLoyaltyPoints command if points > 0 and customer root is known
        if completed.loyalty_points_earned > 0 && !completed.customer_root.is_empty() {
            let customer_root = Some(ProtoUuid {
                value: completed.customer_root.clone(),
            });

            let points_cmd = AddLoyaltyPoints {
                points: completed.loyalty_points_earned,
                reason: format!("order:{}", correlation_id),
            };

            commands.push(build_command_book(
                CUSTOMER_DOMAIN,
                customer_root,
                correlation_id,
                "type.examples/examples.AddLoyaltyPoints",
                &points_cmd,
            ));
        }

        // Generate CommitReservation commands for each item (using cart_root as order_id)
        // The reservations were created with cart_root when items were added to cart
        let order_id = if completed.cart_root.is_empty() {
            // Fallback: no cart_root means no reservations to commit
            return commands;
        } else {
            let cart_uuid = ProtoUuid {
                value: completed.cart_root.clone(),
            };
            root_id_as_string(Some(&cart_uuid))
        };

        for item in &completed.items {
            if item.product_root.is_empty() {
                continue;
            }

            let product_root = Some(ProtoUuid {
                value: item.product_root.clone(),
            });

            let commit_cmd = CommitReservation {
                order_id: order_id.clone(),
            };

            commands.push(build_command_book(
                INVENTORY_DOMAIN,
                product_root,
                correlation_id,
                "type.examples/examples.CommitReservation",
                &commit_cmd,
            ));
        }

        commands
    }

    /// Handle an event book, producing commands for any relevant events.
    pub fn handle(&self, book: &EventBook) -> Vec<CommandBook> {
        process_event_pages(book, |event, root, corr_id| {
            self.process_event(event, root, corr_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{Cover, EventPage};
    use common::proto::LineItem;
    use prost::Message;

    #[test]
    fn test_process_order_completed_with_items() {
        let saga = LoyaltyEarnSaga::new();

        let customer_root_bytes = vec![10; 16];
        let cart_root_bytes = vec![20; 16];
        let product_root_bytes = vec![30; 16];

        let event = OrderCompleted {
            final_total_cents: 5000,
            payment_method: "card".to_string(),
            payment_reference: "PAY-001".to_string(),
            loyalty_points_earned: 50,
            completed_at: None,
            customer_root: customer_root_bytes.clone(),
            cart_root: cart_root_bytes.clone(),
            items: vec![LineItem {
                product_id: "SKU-001".to_string(),
                name: "Widget".to_string(),
                quantity: 2,
                unit_price_cents: 2500,
                product_root: product_root_bytes.clone(),
            }],
        };

        let event_any = prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: event.encode_to_vec(),
        };

        let order_root = ProtoUuid {
            value: vec![1, 2, 3, 4],
        };

        let book = EventBook {
            cover: Some(Cover {
                domain: SOURCE_DOMAIN.to_string(),
                root: Some(order_root),
                correlation_id: "CORR-001".to_string(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                created_at: None,
                sequence: Some(angzarr::proto::event_page::Sequence::Num(1)),
                event: Some(event_any),
            }],
            snapshot_state: None,
        };

        let commands = saga.handle(&book);
        // Should generate: 1 AddLoyaltyPoints + 1 CommitReservation
        assert_eq!(commands.len(), 2);

        // First command: AddLoyaltyPoints
        let points_cmd = &commands[0];
        assert_eq!(points_cmd.cover.as_ref().unwrap().domain, CUSTOMER_DOMAIN);
        assert_eq!(
            points_cmd
                .cover
                .as_ref()
                .unwrap()
                .root
                .as_ref()
                .unwrap()
                .value,
            customer_root_bytes
        );
        let cmd_any = points_cmd.pages[0].command.as_ref().unwrap();
        let add_points = AddLoyaltyPoints::decode(cmd_any.value.as_slice()).expect("Should decode");
        assert_eq!(add_points.points, 50);

        // Second command: CommitReservation
        let commit_cmd = &commands[1];
        assert_eq!(commit_cmd.cover.as_ref().unwrap().domain, INVENTORY_DOMAIN);
        assert_eq!(
            commit_cmd
                .cover
                .as_ref()
                .unwrap()
                .root
                .as_ref()
                .unwrap()
                .value,
            product_root_bytes
        );
        let cmd_any = commit_cmd.pages[0].command.as_ref().unwrap();
        let commit = CommitReservation::decode(cmd_any.value.as_slice()).expect("Should decode");
        // order_id should be derived from cart_root
        assert!(!commit.order_id.is_empty());
    }

    #[test]
    fn test_ignore_zero_points_but_commit_inventory() {
        let saga = LoyaltyEarnSaga::new();

        let cart_root_bytes = vec![20; 16];
        let product_root_bytes = vec![30; 16];

        let event = OrderCompleted {
            final_total_cents: 50, // Only 50 cents = 0 points
            payment_method: "card".to_string(),
            payment_reference: "PAY-002".to_string(),
            loyalty_points_earned: 0,
            completed_at: None,
            customer_root: vec![],
            cart_root: cart_root_bytes,
            items: vec![LineItem {
                product_id: "SKU-001".to_string(),
                name: "Widget".to_string(),
                quantity: 1,
                unit_price_cents: 50,
                product_root: product_root_bytes,
            }],
        };

        let event_any = prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: event.encode_to_vec(),
        };

        let book = EventBook {
            cover: Some(Cover {
                domain: SOURCE_DOMAIN.to_string(),
                root: None,
                correlation_id: "CORR-002".to_string(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                created_at: None,
                sequence: Some(angzarr::proto::event_page::Sequence::Num(1)),
                event: Some(event_any),
            }],
            snapshot_state: None,
        };

        let commands = saga.handle(&book);
        // Should only generate CommitReservation (no loyalty points)
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].cover.as_ref().unwrap().domain, INVENTORY_DOMAIN);
    }

    #[test]
    fn test_no_commands_when_no_cart_root_and_no_points() {
        let saga = LoyaltyEarnSaga::new();

        let event = OrderCompleted {
            final_total_cents: 50,
            payment_method: "card".to_string(),
            payment_reference: "PAY-003".to_string(),
            loyalty_points_earned: 0,
            completed_at: None,
            customer_root: vec![],
            cart_root: vec![], // No cart_root = no inventory to commit
            items: vec![],
        };

        let event_any = prost_types::Any {
            type_url: "type.examples/examples.OrderCompleted".to_string(),
            value: event.encode_to_vec(),
        };

        let book = EventBook {
            cover: Some(Cover {
                domain: SOURCE_DOMAIN.to_string(),
                root: None,
                correlation_id: "CORR-003".to_string(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                created_at: None,
                sequence: Some(angzarr::proto::event_page::Sequence::Num(1)),
                event: Some(event_any),
            }],
            snapshot_state: None,
        };

        let commands = saga.handle(&book);
        assert!(commands.is_empty());
    }
}
