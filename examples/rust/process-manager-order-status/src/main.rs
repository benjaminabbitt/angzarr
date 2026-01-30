use process_manager_order_status::OrderStatusProcess;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run_process_manager_server("order-status", "50171", OrderStatusProcess::new()).await
}
