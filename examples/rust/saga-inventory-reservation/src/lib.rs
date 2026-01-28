//! Inventory Reservation Saga - reserves stock when items added to cart.
//!
//! Demonstrates the reservation pattern:
//! - ItemAdded → ReserveStock (reserve inventory for cart item)
//! - ItemRemoved → ReleaseReservation (release when item removed)
//! - QuantityUpdated → Adjust reservation (release old, reserve new)
//! - CartCleared → ReleaseReservation for all items (release all when cart cleared)
//!
//! The cart's root ID is used as the order_id for reservations,
//! allowing the inventory to track which cart holds each reservation.
//!
//! Note: OrderCompleted → CommitReservation is handled separately by saga-loyalty-earn
//! or a dedicated order-domain saga, since this saga subscribes to cart events only.

use angzarr::proto::{CommandBook, EventBook, Uuid as ProtoUuid};
use common::proto::{
    CartCleared, ItemAdded, ItemRemoved, QuantityUpdated, ReleaseReservation, ReserveStock,
};
use common::{build_command_book, decode_event, process_event_pages, root_id_as_string};

pub const SAGA_NAME: &str = "inventory-reservation";
pub const SOURCE_DOMAIN: &str = "cart";
pub const TARGET_DOMAIN: &str = "inventory";

/// Namespace UUID for generating deterministic product UUIDs.
/// Uses UUID v5 with this namespace + product_id string.
const PRODUCT_UUID_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Generate a deterministic UUID for a product based on its ID.
///
/// Uses UUID v5 (SHA-1 based) to ensure the same product_id always
/// maps to the same inventory aggregate root.
fn product_root(product_id: &str) -> ProtoUuid {
    let uuid = uuid::Uuid::new_v5(&PRODUCT_UUID_NAMESPACE, product_id.as_bytes());
    ProtoUuid {
        value: uuid.as_bytes().to_vec(),
    }
}

/// Inventory Reservation Saga implementation.
pub struct InventoryReservationSaga;
common::define_saga!(InventoryReservationSaga);

impl InventoryReservationSaga {
    fn process_event(
        &self,
        event: &prost_types::Any,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        // ItemAdded → ReserveStock
        if let Some(added) = decode_event::<ItemAdded>(event, "ItemAdded") {
            return self.handle_item_added(&added, source_root, correlation_id);
        }

        // ItemRemoved → ReleaseReservation
        if let Some(removed) = decode_event::<ItemRemoved>(event, "ItemRemoved") {
            return self.handle_item_removed(&removed, source_root, correlation_id);
        }

        // QuantityUpdated → Release old quantity, reserve new
        // For simplicity, we release the old reservation and create a new one.
        // A more sophisticated approach would adjust the existing reservation.
        if let Some(updated) = decode_event::<QuantityUpdated>(event, "QuantityUpdated") {
            return self.handle_quantity_updated(&updated, source_root, correlation_id);
        }

        // CartCleared → Release all reservations
        if let Some(cleared) = decode_event::<CartCleared>(event, "CartCleared") {
            return self.handle_cart_cleared(&cleared, source_root, correlation_id);
        }

        vec![]
    }

    fn handle_item_added(
        &self,
        event: &ItemAdded,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        let cart_id = root_id_as_string(source_root);

        let cmd = ReserveStock {
            quantity: event.quantity,
            order_id: cart_id,
        };

        vec![build_command_book(
            TARGET_DOMAIN,
            Some(product_root(&event.product_id)),
            correlation_id,
            "type.examples/examples.ReserveStock",
            &cmd,
        )]
    }

    fn handle_item_removed(
        &self,
        event: &ItemRemoved,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        let cart_id = root_id_as_string(source_root);

        let cmd = ReleaseReservation { order_id: cart_id };

        vec![build_command_book(
            TARGET_DOMAIN,
            Some(product_root(&event.product_id)),
            correlation_id,
            "type.examples/examples.ReleaseReservation",
            &cmd,
        )]
    }

    fn handle_quantity_updated(
        &self,
        event: &QuantityUpdated,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        let cart_id = root_id_as_string(source_root);
        let product_uuid = product_root(&event.product_id);

        // Release old reservation, then reserve new quantity.
        // Note: This could fail if old reservation doesn't exist (idempotency issue).
        // A production system might use a single "AdjustReservation" command.
        let release_cmd = ReleaseReservation {
            order_id: cart_id.clone(),
        };

        let reserve_cmd = ReserveStock {
            quantity: event.new_quantity,
            order_id: cart_id,
        };

        vec![
            build_command_book(
                TARGET_DOMAIN,
                Some(product_uuid.clone()),
                correlation_id,
                "type.examples/examples.ReleaseReservation",
                &release_cmd,
            ),
            build_command_book(
                TARGET_DOMAIN,
                Some(product_uuid),
                correlation_id,
                "type.examples/examples.ReserveStock",
                &reserve_cmd,
            ),
        ]
    }

    fn handle_cart_cleared(
        &self,
        event: &CartCleared,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        let cart_id = root_id_as_string(source_root);

        // Release reservation for each item that was in the cart
        event
            .items
            .iter()
            .map(|item| {
                let release_cmd = ReleaseReservation {
                    order_id: cart_id.clone(),
                };
                build_command_book(
                    TARGET_DOMAIN,
                    Some(product_root(&item.product_id)),
                    correlation_id,
                    "type.examples/examples.ReleaseReservation",
                    &release_cmd,
                )
            })
            .collect()
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
    use angzarr::proto::{event_page::Sequence, Cover, EventPage};
    use common::proto::CartItem;
    use prost::Message;

    fn make_event_book(event_type: &str, event_data: Vec<u8>) -> EventBook {
        let root = uuid::Uuid::new_v4();
        EventBook {
            cover: Some(Cover {
                domain: SOURCE_DOMAIN.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-correlation".to_string(),
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: format!("type.examples/examples.{}", event_type),
                    value: event_data,
                }),
                created_at: None,
            }],
            snapshot: None,
            snapshot_state: None,
        }
    }

    #[test]
    fn test_item_added_generates_reserve_stock() {
        let saga = InventoryReservationSaga;

        let event = ItemAdded {
            product_id: "SKU-001".to_string(),
            name: "Widget".to_string(),
            quantity: 5,
            unit_price_cents: 1000,
            new_subtotal: 5000,
            added_at: None,
        };

        let book = make_event_book("ItemAdded", event.encode_to_vec());
        let commands = saga.handle(&book);

        assert_eq!(commands.len(), 1);
        let cmd = &commands[0];
        assert_eq!(
            cmd.cover.as_ref().unwrap().domain,
            TARGET_DOMAIN.to_string()
        );

        let cmd_any = cmd.pages[0].command.as_ref().unwrap();
        assert!(cmd_any.type_url.ends_with("ReserveStock"));

        let reserve = ReserveStock::decode(cmd_any.value.as_slice()).unwrap();
        assert_eq!(reserve.quantity, 5);
    }

    #[test]
    fn test_item_removed_generates_release_reservation() {
        let saga = InventoryReservationSaga;

        let event = ItemRemoved {
            product_id: "SKU-001".to_string(),
            quantity: 5,
            new_subtotal: 0,
            removed_at: None,
        };

        let book = make_event_book("ItemRemoved", event.encode_to_vec());
        let commands = saga.handle(&book);

        assert_eq!(commands.len(), 1);
        let cmd_any = commands[0].pages[0].command.as_ref().unwrap();
        assert!(cmd_any.type_url.ends_with("ReleaseReservation"));
    }

    #[test]
    fn test_quantity_updated_generates_release_and_reserve() {
        let saga = InventoryReservationSaga;

        let event = QuantityUpdated {
            product_id: "SKU-001".to_string(),
            old_quantity: 3,
            new_quantity: 7,
            new_subtotal: 7000,
            updated_at: None,
        };

        let book = make_event_book("QuantityUpdated", event.encode_to_vec());
        let commands = saga.handle(&book);

        assert_eq!(commands.len(), 2);

        let release_cmd = commands[0].pages[0].command.as_ref().unwrap();
        assert!(release_cmd.type_url.ends_with("ReleaseReservation"));

        let reserve_cmd = commands[1].pages[0].command.as_ref().unwrap();
        assert!(reserve_cmd.type_url.ends_with("ReserveStock"));

        let reserve = ReserveStock::decode(reserve_cmd.value.as_slice()).unwrap();
        assert_eq!(reserve.quantity, 7);
    }

    #[test]
    fn test_deterministic_product_root() {
        let root1 = product_root("SKU-001");
        let root2 = product_root("SKU-001");
        let root3 = product_root("SKU-002");

        assert_eq!(root1.value, root2.value);
        assert_ne!(root1.value, root3.value);
    }

    #[test]
    fn test_cart_cleared_releases_all_items() {
        let saga = InventoryReservationSaga;

        let event = CartCleared {
            new_subtotal: 0,
            cleared_at: None,
            items: vec![
                CartItem {
                    product_id: "SKU-001".to_string(),
                    name: "Widget".to_string(),
                    quantity: 2,
                    unit_price_cents: 1000,
                },
                CartItem {
                    product_id: "SKU-002".to_string(),
                    name: "Gadget".to_string(),
                    quantity: 3,
                    unit_price_cents: 2000,
                },
            ],
        };

        let book = make_event_book("CartCleared", event.encode_to_vec());
        let commands = saga.handle(&book);

        // Should generate one ReleaseReservation per item
        assert_eq!(commands.len(), 2);

        for cmd in &commands {
            let cmd_any = cmd.pages[0].command.as_ref().unwrap();
            assert!(cmd_any.type_url.ends_with("ReleaseReservation"));
        }
    }

    #[test]
    fn test_ignores_unrelated_events() {
        let saga = InventoryReservationSaga;

        let book = make_event_book(
            "SomeOtherEvent",
            vec![1, 2, 3], // arbitrary data
        );
        let commands = saga.handle(&book);

        assert!(commands.is_empty());
    }
}
