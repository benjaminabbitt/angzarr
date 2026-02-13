//! Player bounded context gRPC server.

use agg_player::{handlers, state};
use angzarr_client::{run_aggregate_server, CommandRouter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = CommandRouter::new("player", state::rebuild_state)
        .on("RegisterPlayer", handlers::handle_register_player)
        .on("DepositFunds", handlers::handle_deposit_funds)
        .on("WithdrawFunds", handlers::handle_withdraw_funds)
        .on("ReserveFunds", handlers::handle_reserve_funds)
        .on("ReleaseFunds", handlers::handle_release_funds);

    run_aggregate_server("player", 50001, router)
        .await
        .expect("Server failed");
}
