//! Fulfillment bounded context gRPC server.

use fulfillment::FulfillmentLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("fulfillment", "50058", FulfillmentLogic::new()).await
}
