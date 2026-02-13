//! Hand bounded context gRPC server.

use agg_hand::{handlers, state};

use angzarr_client::{run_aggregate_server, CommandRouter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = CommandRouter::new("hand", state::rebuild_state)
        .on("DealCards", handlers::handle_deal_cards)
        .on("PostBlind", handlers::handle_post_blind)
        .on("PlayerAction", handlers::handle_player_action)
        .on("DealCommunityCards", handlers::handle_deal_community_cards)
        .on("RequestDraw", handlers::handle_request_draw)
        .on("RevealCards", handlers::handle_reveal_cards)
        .on("AwardPot", handlers::handle_award_pot);

    run_aggregate_server("hand", 50003, router)
        .await
        .expect("Server failed");
}
