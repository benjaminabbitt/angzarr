//! Table bounded context gRPC server.

use agg_table::{handlers, state};

use angzarr_client::{run_aggregate_server, CommandRouter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = CommandRouter::new("table", state::rebuild_state)
        .on("CreateTable", handlers::handle_create_table)
        .on("JoinTable", handlers::handle_join_table)
        .on("LeaveTable", handlers::handle_leave_table)
        .on("StartHand", handlers::handle_start_hand)
        .on("EndHand", handlers::handle_end_hand);

    run_aggregate_server("table", 50002, router)
        .await
        .expect("Server failed");
}
