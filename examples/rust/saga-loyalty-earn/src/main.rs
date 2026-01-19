//! Loyalty Earn Saga gRPC server.

use saga_loyalty_earn::LoyaltyEarnSaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("loyalty-earn", "50113", LoyaltyEarnSaga::new()).await
}
