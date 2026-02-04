//! Order-Inventory Saga gRPC server.

use saga_order_inventory::OrderInventorySaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("order-inventory", "50126", OrderInventorySaga::new()).await
}
