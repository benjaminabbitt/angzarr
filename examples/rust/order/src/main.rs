//! Order bounded context gRPC server.

use order::OrderLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("order", "50056", OrderLogic::new()).await
}
