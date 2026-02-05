//! Inventory Projector - builds read model from inventory events.
//!
//! Listens to inventory domain events and logs stock level changes.
//! Demonstrates the projector pattern for building read models.

use angzarr::proto::{ComponentDescriptor, EventBook, Projection, Target};
use common::proto::{
    LowStockAlert, ReservationCommitted, ReservationReleased, StockInitialized, StockReceived,
    StockReserved,
};
use common::{decode_event, ProjectorLogic, ProtoTypeName};
use tonic::Status;
use tracing::info;

pub const PROJECTOR_NAME: &str = "prj-inventory";
const SOURCE_DOMAIN: &str = "inventory";

/// Inventory Projector implementation.
///
/// Consumes inventory domain events and logs stock level changes.
pub struct InventoryProjector;

impl InventoryProjector {
    pub fn new() -> Self {
        Self
    }

    fn build_descriptor() -> ComponentDescriptor {
        ComponentDescriptor {
            component_type: "projector".to_string(),
            name: PROJECTOR_NAME.to_string(),
            inputs: vec![Target {
                domain: SOURCE_DOMAIN.to_string(),
                types: vec![
                    StockInitialized::TYPE_NAME.to_string(),
                    StockReceived::TYPE_NAME.to_string(),
                    StockReserved::TYPE_NAME.to_string(),
                    ReservationReleased::TYPE_NAME.to_string(),
                    ReservationCommitted::TYPE_NAME.to_string(),
                    LowStockAlert::TYPE_NAME.to_string(),
                ],
            }],
            outputs: vec![],
        }
    }
}

impl Default for InventoryProjector {
    fn default() -> Self {
        Self::new()
    }
}

/// ProjectorLogic for standalone server binary (common::run_projector_server)
#[tonic::async_trait]
impl ProjectorLogic for InventoryProjector {
    fn descriptor(&self) -> ComponentDescriptor {
        Self::build_descriptor()
    }

    async fn handle(&self, book: &EventBook) -> Result<Option<Projection>, Status> {
        for page in &book.pages {
            if let Some(event) = &page.event {
                process_event(event);
            }
        }
        Ok(None)
    }
}

/// ProjectorHandler for e2e test runtime (angzarr::standalone::RuntimeBuilder)
#[cfg(feature = "standalone")]
#[tonic::async_trait]
impl angzarr::standalone::ProjectorHandler for InventoryProjector {
    async fn handle(
        &self,
        book: &EventBook,
        mode: angzarr::standalone::ProjectionMode,
    ) -> Result<Projection, Status> {
        if mode == angzarr::standalone::ProjectionMode::Execute {
            for page in &book.pages {
                if let Some(event) = &page.event {
                    process_event(event);
                }
            }
        }
        Ok(Projection::default())
    }
}

fn process_event(event: &prost_types::Any) {
    if let Some(e) = decode_event::<StockInitialized>(event, StockInitialized::TYPE_NAME) {
        info!(
            event = StockInitialized::TYPE_NAME,
            product_id = %e.product_id,
            quantity = e.quantity,
            threshold = e.low_stock_threshold,
            "inventory_projected"
        );
    } else if let Some(e) = decode_event::<StockReceived>(event, StockReceived::TYPE_NAME) {
        info!(
            event = StockReceived::TYPE_NAME,
            quantity = e.quantity,
            new_on_hand = e.new_on_hand,
            reference = %e.reference,
            "inventory_projected"
        );
    } else if let Some(e) = decode_event::<StockReserved>(event, StockReserved::TYPE_NAME) {
        info!(
            event = StockReserved::TYPE_NAME,
            order_id = %e.order_id,
            quantity = e.quantity,
            new_available = e.new_available,
            new_reserved = e.new_reserved,
            "inventory_projected"
        );
    } else if let Some(e) = decode_event::<ReservationReleased>(event, ReservationReleased::TYPE_NAME) {
        info!(
            event = ReservationReleased::TYPE_NAME,
            order_id = %e.order_id,
            quantity = e.quantity,
            new_available = e.new_available,
            "inventory_projected"
        );
    } else if let Some(e) = decode_event::<ReservationCommitted>(event, ReservationCommitted::TYPE_NAME) {
        info!(
            event = ReservationCommitted::TYPE_NAME,
            order_id = %e.order_id,
            quantity = e.quantity,
            new_on_hand = e.new_on_hand,
            "inventory_projected"
        );
    } else if let Some(e) = decode_event::<LowStockAlert>(event, LowStockAlert::TYPE_NAME) {
        info!(
            event = LowStockAlert::TYPE_NAME,
            product_id = %e.product_id,
            available = e.available,
            threshold = e.threshold,
            "inventory_projected"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projector_name() {
        assert_eq!(PROJECTOR_NAME, "prj-inventory");
    }

    #[test]
    fn test_descriptor() {
        let projector = InventoryProjector::new();
        let desc = projector.descriptor();
        assert_eq!(desc.component_type, "projector");
        assert_eq!(desc.name, "prj-inventory");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].domain, "inventory");
    }
}
