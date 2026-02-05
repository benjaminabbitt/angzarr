//! Order-Fulfillment Saga gRPC server.

use sag_order_fulfillment::OrderFulfillmentSaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("order-fulfillment", "50123", OrderFulfillmentSaga::new()).await
}
