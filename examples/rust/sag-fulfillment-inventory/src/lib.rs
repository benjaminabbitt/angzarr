//! Fulfillment-Inventory Saga - commits inventory reservations when shipments are shipped.
//!
//! Bridges: fulfillment -> inventory
//! Listens to Shipped events and generates CommitReservation commands for each line item.

use angzarr::proto::{CommandBook, ComponentDescriptor, EventBook, Uuid as ProtoUuid};
use common::proto::{CommitReservation, Shipped};
use common::{build_command_book, decode_event, EventRouter, SagaLogic};

const TARGET_DOMAIN: &str = "inventory";

/// Generate a deterministic UUID for a product based on its ID.
fn product_root(product_id: &str) -> ProtoUuid {
    let uuid = common::identity::inventory_product_root(product_id);
    ProtoUuid {
        value: uuid.as_bytes().to_vec(),
    }
}

/// Fulfillment-Inventory Saga implementation.
pub struct FulfillmentInventorySaga {
    router: EventRouter,
}

impl FulfillmentInventorySaga {
    pub fn new() -> Self {
        Self {
            router: EventRouter::new("sag-fulfillment-inventory", "fulfillment")
                .sends(TARGET_DOMAIN, "CommitReservation")
                .on("Shipped", handle_shipped),
        }
    }
}

impl Default for FulfillmentInventorySaga {
    fn default() -> Self {
        Self::new()
    }
}

fn handle_shipped(
    event: &prost_types::Any,
    _source_root: Option<&ProtoUuid>,
    correlation_id: &str,
) -> Vec<CommandBook> {
    let Some(shipped) = decode_event::<Shipped>(event, "Shipped") else {
        return vec![];
    };

    // Commit reservation for each line item
    shipped
        .items
        .iter()
        .map(|item| {
            let cmd = CommitReservation {
                order_id: shipped.order_id.clone(),
            };
            build_command_book(
                TARGET_DOMAIN,
                Some(product_root(&item.product_id)),
                correlation_id,
                "type.examples/examples.CommitReservation",
                &cmd,
            )
        })
        .collect()
}

impl SagaLogic for FulfillmentInventorySaga {
    fn descriptor(&self) -> ComponentDescriptor {
        self.router.descriptor()
    }

    fn execute(
        &self,
        source: &EventBook,
        _destinations: &[EventBook],
    ) -> Vec<CommandBook> {
        self.router.dispatch(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventPage};
    use common::proto::LineItem;
    use prost::Message;

    fn make_event_book(event_type: &str, event_data: Vec<u8>) -> EventBook {
        let root = uuid::Uuid::new_v4();
        EventBook {
            cover: Some(Cover {
                domain: "fulfillment".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-correlation".to_string(),
                edition: None,
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
    fn test_shipped_commits_reservation_for_each_item() {
        let saga = FulfillmentInventorySaga::new();

        let event = Shipped {
            carrier: "UPS".to_string(),
            tracking_number: "1Z999".to_string(),
            shipped_at: None,
            items: vec![
                LineItem {
                    product_id: "SKU-001".to_string(),
                    name: "Widget".to_string(),
                    quantity: 5,
                    unit_price_cents: 1000,
                    ..Default::default()
                },
                LineItem {
                    product_id: "SKU-002".to_string(),
                    name: "Gadget".to_string(),
                    quantity: 3,
                    unit_price_cents: 2000,
                    ..Default::default()
                },
            ],
            order_id: "order-123".to_string(),
        };

        let book = make_event_book("Shipped", event.encode_to_vec());
        let commands = saga.execute(&book, &[]);

        assert_eq!(commands.len(), 2);

        // Both commands target inventory domain
        for cmd in &commands {
            assert_eq!(cmd.cover.as_ref().unwrap().domain, TARGET_DOMAIN);
            let cmd_any = cmd.pages[0].command.as_ref().unwrap();
            assert!(cmd_any.type_url.ends_with("CommitReservation"));
        }
    }

    #[test]
    fn test_shipped_with_no_items_generates_no_commands() {
        let saga = FulfillmentInventorySaga::new();

        let event = Shipped {
            carrier: "UPS".to_string(),
            tracking_number: "1Z999".to_string(),
            shipped_at: None,
            items: vec![],
            order_id: "order-123".to_string(),
        };

        let book = make_event_book("Shipped", event.encode_to_vec());
        let commands = saga.execute(&book, &[]);

        assert!(commands.is_empty());
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
    fn test_ignores_unrelated_events() {
        let saga = FulfillmentInventorySaga::new();

        let book = make_event_book("SomeOtherEvent", vec![1, 2, 3]);
        let commands = saga.execute(&book, &[]);

        assert!(commands.is_empty());
    }
}
