//! Inventory Reservation Saga gRPC server.

use saga_inventory_reservation::InventoryReservationSaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("inventory-reservation", "50126", InventoryReservationSaga::new()).await
}
