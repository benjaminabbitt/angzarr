//! Customer bounded context gRPC server.

use customer::CustomerLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("customer", "50055", CustomerLogic::new()).await
}
