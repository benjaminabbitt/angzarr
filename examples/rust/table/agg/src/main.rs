//! Table bounded context gRPC server.

use agg_table::TableHandler;
use angzarr_client::{run_command_handler_server, CommandHandlerRouter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = CommandHandlerRouter::new("table", "table", TableHandler::new());

    run_command_handler_server("table", 50002, router)
        .await
        .expect("Server failed");
}
