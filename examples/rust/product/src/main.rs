//! Product bounded context gRPC server.

use product::ProductLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("product", "50053", ProductLogic::new()).await
}
