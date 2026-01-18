//! Fulfillment Saga gRPC server.

use saga_fulfillment::FulfillmentSaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("fulfillment", "50061", FulfillmentSaga::new()).await
}
