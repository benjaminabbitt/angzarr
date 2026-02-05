//! Fulfillment bounded context gRPC server.

use agg_fulfillment::FulfillmentLogic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_aggregate_server("fulfillment", "50006", FulfillmentLogic::new()).await
}
