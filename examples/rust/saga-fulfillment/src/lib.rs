//! Fulfillment Saga - creates shipments when orders complete.
//!
//! Listens to OrderCompleted events and generates CreateShipment commands.

use angzarr::proto::{CommandBook, EventBook, Uuid as ProtoUuid};
use common::proto::{CreateShipment, OrderCompleted};
use common::{build_command_book, decode_event, process_event_pages, root_id_as_string};

pub const SAGA_NAME: &str = "fulfillment";
pub const SOURCE_DOMAIN: &str = "order";
pub const TARGET_DOMAIN: &str = "fulfillment";

/// Fulfillment Saga implementation.
pub struct FulfillmentSaga;
common::define_saga!(FulfillmentSaga);

impl FulfillmentSaga {
    fn process_event(
        &self,
        event: &prost_types::Any,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        let Some(_) = decode_event::<OrderCompleted>(event, "OrderCompleted") else {
            return vec![];
        };

        let order_id = root_id_as_string(source_root);

        let cmd = CreateShipment {
            order_id: order_id.clone(),
        };

        vec![build_command_book(
            TARGET_DOMAIN,
            source_root.cloned(),
            correlation_id,
            "type.examples/examples.CreateShipment",
            &cmd,
        )]
    }

    /// Handle an event book, producing commands for any relevant events.
    pub fn handle(&self, book: &EventBook) -> Vec<CommandBook> {
        process_event_pages(book, |event, root, corr_id| {
            self.process_event(event, root, corr_id)
        })
    }
}
