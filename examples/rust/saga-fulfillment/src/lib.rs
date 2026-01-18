//! Fulfillment Saga - creates shipments when orders complete.
//!
//! Listens to OrderCompleted events and generates CreateShipment commands.

use prost::Message;

use angzarr::proto::{CommandBook, CommandPage, Cover, EventBook, Uuid as ProtoUuid};
use common::proto::{CreateShipment, OrderCompleted};

pub const SAGA_NAME: &str = "fulfillment";
pub const SOURCE_DOMAIN: &str = "order";
pub const TARGET_DOMAIN: &str = "fulfillment";

/// Fulfillment Saga implementation.
pub struct FulfillmentSaga;

impl FulfillmentSaga {
    pub fn new() -> Self {
        Self
    }

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

        // Verify it decodes correctly
        OrderCompleted::decode(event.value.as_slice()).ok()?;

        // Use root ID as order ID (convert bytes to UTF-8 string)
        let order_id = source_root
            .map(|r| String::from_utf8_lossy(&r.value).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let cmd = CreateShipment {
            order_id: order_id.clone(),
        };

        let cmd_any = prost_types::Any {
            type_url: "type.examples/examples.CreateShipment".to_string(),
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
            saga_origin: None,
            auto_resequence: false,
            fact: false,
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

impl Default for FulfillmentSaga {
    fn default() -> Self {
        Self::new()
    }
}

impl common::SagaLogic for FulfillmentSaga {
    fn handle(&self, book: &EventBook) -> Vec<CommandBook> {
        self.handle(book)
    }
}
