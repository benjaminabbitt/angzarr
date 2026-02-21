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
        .on("examples.CreateTable", handlers::handle_create_table)
        .on("examples.JoinTable", handlers::handle_join_table)
        .on("examples.LeaveTable", handlers::handle_leave_table)
        .on("examples.StartHand", handlers::handle_start_hand)
        .on("examples.EndHand", handlers::handle_end_hand);

    run_aggregate_server("table", 50002, router)
        .await
        .expect("Server failed");
}
