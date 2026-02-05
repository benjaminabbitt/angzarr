//! Fulfillment-Inventory Saga gRPC server.

use sag_fulfillment_inventory::FulfillmentInventorySaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("fulfillment-inventory", "50127", FulfillmentInventorySaga::new()).await
}
