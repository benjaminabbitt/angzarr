//! Inventory bounded context gRPC server.

use agg_inventory::InventoryLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("inventory", "50003", InventoryLogic::new()).await
}
