//! Order Cancellation Saga - handles compensation when orders are cancelled.
//!
//! Listens to OrderCancelled events and generates:
//! - ReleaseReservation command (to inventory)
//! - AddLoyaltyPoints command (to customer, if points were used)

use angzarr::proto::{CommandBook, EventBook, Uuid as ProtoUuid};
use common::proto::{AddLoyaltyPoints, OrderCancelled, ReleaseReservation};
use common::{build_command_book, decode_event, process_event_pages, root_id_as_string};

pub const SAGA_NAME: &str = "cancellation";
pub const SOURCE_DOMAIN: &str = "order";
pub const INVENTORY_DOMAIN: &str = "inventory";
pub const CUSTOMER_DOMAIN: &str = "customer";

/// Order Cancellation Saga implementation.
pub struct CancellationSaga;
common::define_saga!(CancellationSaga);

impl CancellationSaga {
    fn process_event(
        &self,
        event: &prost_types::Any,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        // Only process OrderCancelled events
        let Some(cancelled) = decode_event::<OrderCancelled>(event, "OrderCancelled") else {
            return vec![];
        };

        // Use cart_root as order_id for inventory reservations (they were created with cart_root)
        let order_id = if cancelled.cart_root.is_empty() {
            root_id_as_string(source_root)
        } else {
            let cart_uuid = ProtoUuid {
                value: cancelled.cart_root.clone(),
            };
            root_id_as_string(Some(&cart_uuid))
        };

        let mut commands = Vec::new();

        // Release inventory reservation per product
        for item in &cancelled.items {
            let product_root = if item.product_root.is_empty() {
                None
            } else {
                Some(ProtoUuid {
                    value: item.product_root.clone(),
                })
            };

            let release_cmd = ReleaseReservation {
                order_id: order_id.clone(),
            };

            commands.push(build_command_book(
                INVENTORY_DOMAIN,
                product_root,
                correlation_id,
                "type.examples/examples.ReleaseReservation",
                &release_cmd,
            ));
        }

        // Return loyalty points if any were used
        if cancelled.loyalty_points_used > 0 {
            let customer_root = if cancelled.customer_root.is_empty() {
                None
            } else {
                Some(ProtoUuid {
                    value: cancelled.customer_root.clone(),
                })
            };

            let points_cmd = AddLoyaltyPoints {
                points: cancelled.loyalty_points_used,
                reason: format!("Refund for cancelled order {}", order_id),
            };

            commands.push(build_command_book(
                CUSTOMER_DOMAIN,
                customer_root,
                correlation_id,
                "type.examples/examples.AddLoyaltyPoints",
                &points_cmd,
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
