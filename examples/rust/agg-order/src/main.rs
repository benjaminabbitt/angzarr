//! Order bounded context gRPC server.

use agg_order::OrderLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("order", "50000", OrderLogic::new()).await
}
