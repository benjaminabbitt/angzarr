//! Order Cancellation Saga gRPC server.

use saga_cancellation::CancellationSaga;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_saga_server("cancellation", "50133", CancellationSaga::new()).await
}
