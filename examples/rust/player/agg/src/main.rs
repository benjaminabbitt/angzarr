//! Player bounded context gRPC server.

use agg_player::PlayerHandler;
use angzarr_client::{run_command_handler_server, CommandHandlerRouter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // docs:start:command_router
    let router = CommandHandlerRouter::new("player", "player", PlayerHandler::new());
    // docs:end:command_router

    run_command_handler_server("player", 50001, router)
        .await
        .expect("Server failed");
}
