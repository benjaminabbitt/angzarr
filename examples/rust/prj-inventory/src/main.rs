//! Inventory Projector server binary.

use prj_inventory::{InventoryProjector, PROJECTOR_NAME};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_projector_server(PROJECTOR_NAME, "inventory", "50160", InventoryProjector::new())
        .await
}
