//! Order-Inventory Saga - reserves stock when orders are created.
//!
//! Bridges: order → inventory
//! Listens to OrderCreated events and generates ReserveStock commands for each line item.

use angzarr::proto::{CommandBook, ComponentDescriptor, EventBook, Uuid as ProtoUuid};
use common::proto::{OrderCreated, ReserveStock};
use common::{
    build_command_book, decode_event, root_id_as_string, Dispatcher, ProtoTypeName, Router,
    SagaEventHandler, SagaLogic, SAGA,
};

const TARGET_DOMAIN: &str = "inventory";

/// Generate a deterministic UUID for a product based on its ID.
fn product_root(product_id: &str) -> ProtoUuid {
    let uuid = common::identity::inventory_product_root(product_id);
    ProtoUuid {
        value: uuid.as_bytes().to_vec(),
    }
}

/// Order-Inventory Saga implementation.
pub struct OrderInventorySaga {
    router: Router<SagaEventHandler>,
}

impl OrderInventorySaga {
    pub fn new() -> Self {
        Self {
            router: Router::new("sag-order-inventory", SAGA)
                .with(Dispatcher::new("order").on(OrderCreated::TYPE_NAME, handle_order_created)),
        }
    }
}

impl Default for OrderInventorySaga {
    fn default() -> Self {
        Self::new()
    }
}

fn handle_order_created(
    event: &prost_types::Any,
    source_root: Option<&ProtoUuid>,
    correlation_id: &str,
) -> Vec<CommandBook> {
    let Some(created) = decode_event::<OrderCreated>(event, OrderCreated::TYPE_NAME) else {
        return vec![];
    };

    let order_id = root_id_as_string(source_root);

    // Reserve stock for each line item
    created
        .items
        .iter()
        .map(|item| {
            let cmd = ReserveStock {
                quantity: item.quantity,
                order_id: order_id.clone(),
            };
            build_command_book(
                TARGET_DOMAIN,
                Some(product_root(&item.product_id)),
                correlation_id,
                &ReserveStock::type_url(),
                &cmd,
            )
        })
        .collect()
}

impl SagaLogic for OrderInventorySaga {
    fn descriptor(&self) -> ComponentDescriptor {
        self.router.descriptor()
    }

    fn execute(&self, source: &EventBook, _destinations: &[EventBook]) -> Vec<CommandBook> {
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
                domain: "order".to_string(),
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
            ..Default::default()
        }
    }

    #[test]
    fn test_order_created_reserves_stock_for_each_item() {
        let saga = OrderInventorySaga::new();

        let event = OrderCreated {
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
            subtotal_cents: 11000,
            ..Default::default()
        };

        let book = make_event_book("OrderCreated", event.encode_to_vec());
        let commands = saga.execute(&book, &[]);

        assert_eq!(commands.len(), 2);

        // First command: reserve 5 units of SKU-001
        let cmd1 = &commands[0];
        assert_eq!(cmd1.cover.as_ref().unwrap().domain, TARGET_DOMAIN);
        let cmd1_any = cmd1.pages[0].command.as_ref().unwrap();
        assert!(cmd1_any.type_url.ends_with("ReserveStock"));
        let reserve1 = ReserveStock::decode(cmd1_any.value.as_slice()).unwrap();
        assert_eq!(reserve1.quantity, 5);

        // Second command: reserve 3 units of SKU-002
        let cmd2_any = commands[1].pages[0].command.as_ref().unwrap();
        let reserve2 = ReserveStock::decode(cmd2_any.value.as_slice()).unwrap();
        assert_eq!(reserve2.quantity, 3);
    }

    #[test]
    fn test_order_with_no_items_generates_no_commands() {
        let saga = OrderInventorySaga::new();

        let event = OrderCreated {
            items: vec![],
            subtotal_cents: 0,
            ..Default::default()
        };

        let book = make_event_book("OrderCreated", event.encode_to_vec());
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
        let saga = OrderInventorySaga::new();

        let book = make_event_book("SomeOtherEvent", vec![1, 2, 3]);
        let commands = saga.execute(&book, &[]);

        assert!(commands.is_empty());
    }
}
