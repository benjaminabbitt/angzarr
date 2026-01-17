//! Order Cancellation Saga - handles compensation when orders are cancelled.
//!
//! Listens to OrderCancelled events and generates:
//! - ReleaseReservation command (to inventory)
//! - AddLoyaltyPoints command (to customer, if points were used)

use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;

use angzarr::interfaces::saga::{Result, Saga};
use angzarr::proto::{CommandBook, CommandPage, Cover, EventBook, Uuid as ProtoUuid};
use common::proto::{AddLoyaltyPoints, OrderCancelled, ReleaseReservation};

pub const SAGA_NAME: &str = "cancellation";
pub const SOURCE_DOMAIN: &str = "order";
pub const INVENTORY_DOMAIN: &str = "inventory";
pub const CUSTOMER_DOMAIN: &str = "customer";

/// Order Cancellation Saga implementation.
pub struct CancellationSaga {
    name: String,
}

impl CancellationSaga {
    pub fn new() -> Self {
        Self {
            name: SAGA_NAME.to_string(),
        }
    }

    fn process_event(
        &self,
        event: &prost_types::Any,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        // Only process OrderCancelled events
        if !event.type_url.ends_with("OrderCancelled") {
            return vec![];
        }

        // Decode the event
        let cancelled = match OrderCancelled::decode(event.value.as_slice()) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        // Use root ID as order ID
        let order_id = source_root
            .map(|r| String::from_utf8_lossy(&r.value).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut commands = Vec::new();

        // Always release inventory reservation
        let release_cmd = ReleaseReservation {
            order_id: order_id.clone(),
        };

        let release_any = prost_types::Any {
            type_url: "type.examples/examples.ReleaseReservation".to_string(),
            value: release_cmd.encode_to_vec(),
        };

        commands.push(CommandBook {
            cover: Some(Cover {
                domain: INVENTORY_DOMAIN.to_string(),
                root: source_root.cloned(),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                synchronous: false,
                command: Some(release_any),
            }],
            correlation_id: correlation_id.to_string(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        });

        // Return loyalty points if any were used
        if cancelled.loyalty_points_used > 0 {
            let points_cmd = AddLoyaltyPoints {
                points: cancelled.loyalty_points_used,
                reason: format!("Refund for cancelled order {}", order_id),
            };

            let points_any = prost_types::Any {
                type_url: "type.examples/examples.AddLoyaltyPoints".to_string(),
                value: points_cmd.encode_to_vec(),
            };

            commands.push(CommandBook {
                cover: Some(Cover {
                    domain: CUSTOMER_DOMAIN.to_string(),
                    root: source_root.cloned(),
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    synchronous: false,
                    command: Some(points_any),
                }],
                correlation_id: correlation_id.to_string(),
                saga_origin: None,
                auto_resequence: false,
                fact: false,
            });
        }

        commands
    }
}

impl Default for CancellationSaga {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Saga for CancellationSaga {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        vec![SOURCE_DOMAIN.to_string()]
    }

    async fn handle(&self, book: &Arc<EventBook>) -> Result<Vec<CommandBook>> {
        let source_root = book.cover.as_ref().and_then(|c| c.root.as_ref());
        let correlation_id = &book.correlation_id;

        let commands: Vec<CommandBook> = book
            .pages
            .iter()
            .flat_map(|page| {
                page.event
                    .as_ref()
                    .map(|e| self.process_event(e, source_root, correlation_id))
                    .unwrap_or_default()
            })
            .collect();

        Ok(commands)
    }

    fn is_synchronous(&self) -> bool {
        false
    }
}

impl CancellationSaga {
    pub fn process_event_public(
        &self,
        event: &prost_types::Any,
        source_root: Option<&ProtoUuid>,
        correlation_id: &str,
    ) -> Vec<CommandBook> {
        self.process_event(event, source_root, correlation_id)
    }
}
