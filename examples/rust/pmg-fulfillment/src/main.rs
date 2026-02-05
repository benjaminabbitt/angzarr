use pmg_fulfillment::OrderFulfillmentProcess;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_process_manager_server("order-fulfillment", "50170", OrderFulfillmentProcess::new())
        .await
}
