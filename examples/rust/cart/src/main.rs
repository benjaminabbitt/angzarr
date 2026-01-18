//! Cart bounded context gRPC server.

use cart::CartLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("cart", "50057", CartLogic::new()).await
}
