//! Order-Fulfillment Saga - creates shipments when orders complete.
//!
//! Bridges: order → fulfillment
//! Listens to OrderCompleted events and generates CreateShipment commands.

use angzarr::proto::{CommandBook, ComponentDescriptor, EventBook, Uuid as ProtoUuid};
use common::proto::{CreateShipment, OrderCompleted};
use common::{build_command_book, decode_event, root_id_as_string, Dispatcher, Router, SagaEventHandler, ProtoTypeName, SagaLogic, SAGA};

const TARGET_DOMAIN: &str = "fulfillment";

/// Order-Fulfillment Saga implementation.
pub struct OrderFulfillmentSaga {
    router: Router<SagaEventHandler>,
}

impl OrderFulfillmentSaga {
    pub fn new() -> Self {
        Self {
            router: Router::new("sag-order-fulfillment", SAGA)
                .with(Dispatcher::new("order").on(OrderCompleted::TYPE_NAME, handle_order_completed)),
        }
    }
}

impl Default for OrderFulfillmentSaga {
    fn default() -> Self {
        Self::new()
    }
}

fn handle_order_completed(
    event: &prost_types::Any,
    source_root: Option<&ProtoUuid>,
    correlation_id: &str,
) -> Vec<CommandBook> {
    let Some(completed) = decode_event::<OrderCompleted>(event, OrderCompleted::TYPE_NAME) else {
        return vec![];
    };

    let order_id = root_id_as_string(source_root);

    let cmd = CreateShipment {
        order_id: order_id.clone(),
        items: completed.items,
    };

    vec![build_command_book(
        TARGET_DOMAIN,
        source_root.cloned(),
        correlation_id,
        &CreateShipment::type_url(),
        &cmd,
    )]
}

impl SagaLogic for OrderFulfillmentSaga {
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
