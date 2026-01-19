//! Inventory bounded context gRPC server.

use inventory_svc::InventoryLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("inventory", "50073", InventoryLogic::new()).await
}
