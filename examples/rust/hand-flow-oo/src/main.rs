//! Process Manager: Hand Flow (OO Pattern)
//!
//! Orchestrates the flow of poker hands by:
//! 1. Subscribing to table and hand domain events
//! 2. Managing hand process state machines
//! 3. Sending commands to drive hands forward
//!
//! This example demonstrates the OO pattern using:
//! - `#[process_manager(name = "...", domain = "...", state = ..., inputs = [...])]` on impl blocks
//! - `#[prepares(EventType)]` on prepare handler methods
//! - `#[handles(EventType)]` on event handler methods
//! - `#[applies(EventType)]` on state applier methods (optional)

use angzarr_client::proto::examples::{
    ActionTaken, BlindPosted, CardsDealt, CommunityCardsDealt, HandStarted, PotAwarded,
};
use angzarr_client::proto::{Cover, EventBook, Uuid};
use angzarr_client::{run_process_manager_server, CommandResult, ProcessManagerResponse};
#[allow(unused_imports)]
use prost::Message;
#[allow(unused_imports)]
use angzarr_macros::{process_manager, prepares, handles, applies};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// The PM's aggregate state (rebuilt from its own events).
/// For simplicity in this example, we use a minimal state.
#[derive(Clone, Default)]
pub struct PMState {
    /// Current hand being tracked (if any).
    pub hand_root: Option<Vec<u8>>,
    /// Whether the hand is in progress.
    pub hand_in_progress: bool,
}

/// Hand Flow Process Manager using OO-style annotations.
pub struct HandFlowPM;

#[process_manager(name = "hand-flow", domain = "hand-flow", state = PMState, inputs = ["table", "hand"])]
impl HandFlowPM {
    // Note: In a real implementation, you would have #[applies] methods to rebuild state
    // from the PM's own events. For this simplified example, we don't persist PM events.

    #[prepares(HandStarted)]
    fn prepare_hand_started(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        event: &HandStarted,
    ) -> Vec<Cover> {
        vec![Cover {
            domain: "hand".to_string(),
            root: Some(Uuid {
                value: event.hand_root.clone(),
            }),
            ..Default::default()
        }]
    }

    #[handles(HandStarted)]
    fn handle_hand_started(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        _event: HandStarted,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // Initialize hand process (not persisted in this simplified version)
        // The saga-table-hand will send DealCards, so we don't emit commands here.
        Ok(ProcessManagerResponse::default())
    }

    #[handles(CardsDealt)]
    fn handle_cards_dealt(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        _event: CardsDealt,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // Post small blind command
        // In a real implementation, we'd track state to know which blind to post.
        // For now, we assume the hand aggregate tracks this.
        Ok(ProcessManagerResponse::default())
    }

    #[handles(BlindPosted)]
    fn handle_blind_posted(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        _event: BlindPosted,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // In a full implementation, we'd check if both blinds are posted
        // and then start the betting round.
        Ok(ProcessManagerResponse::default())
    }

    #[handles(ActionTaken)]
    fn handle_action_taken(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        _event: ActionTaken,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // In a full implementation, we'd check if betting is complete
        // and advance to the next phase.
        Ok(ProcessManagerResponse::default())
    }

    #[handles(CommunityCardsDealt)]
    fn handle_community_dealt(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        _event: CommunityCardsDealt,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // Start new betting round after community cards.
        Ok(ProcessManagerResponse::default())
    }

    #[handles(PotAwarded)]
    fn handle_pot_awarded(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        _event: PotAwarded,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // Hand is complete. Clean up.
        Ok(ProcessManagerResponse::default())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("Starting Hand Flow process manager (OO pattern)");

    let pm = HandFlowPM;
    let router = pm.into_router();

    run_process_manager_server("hand-flow", 50092, router)
        .await
        .expect("Server failed");
}
