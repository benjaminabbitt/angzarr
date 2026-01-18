//! Inventory bounded context gRPC server.

use inventory::InventoryLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("inventory", "50054", InventoryLogic::new()).await
}
