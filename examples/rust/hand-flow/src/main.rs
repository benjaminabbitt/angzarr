//! Process Manager: Hand Flow
//!
//! Orchestrates the flow of poker hands by:
//! 1. Subscribing to table and hand domain events
//! 2. Managing hand process state machines
//! 3. Sending commands to drive hands forward

use angzarr_client::proto::examples::{
    ActionTaken, BlindPosted, CardsDealt, CommunityCardsDealt, HandStarted, PotAwarded,
};
use angzarr_client::proto::{Cover, EventBook, Uuid};
use angzarr_client::{
    run_process_manager_server, CommandResult, ProcessManagerResponse, ProcessManagerRouter,
};
use prost::Message;
use prost_types::Any;
use std::collections::HashMap;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Internal phase tracking for hand orchestration.
#[derive(Clone, Copy, Default, PartialEq)]
enum HandPhase {
    #[default]
    WaitingForStart,
    Dealing,
    PostingBlinds,
    Betting,
    DealingCommunity,
    Showdown,
    Complete,
}

/// Player state within the process manager.
#[derive(Clone, Default)]
struct PlayerState {
    player_root: Vec<u8>,
    position: i32,
    stack: i64,
    bet_this_round: i64,
    has_acted: bool,
    has_folded: bool,
    is_all_in: bool,
}

/// Process manager state for a single hand.
#[derive(Clone, Default)]
struct HandProcess {
    hand_root: Vec<u8>,
    table_root: Vec<u8>,
    hand_number: i64,
    game_variant: i32,

    phase: HandPhase,
    betting_phase: i32,

    players: HashMap<i32, PlayerState>,
    active_positions: Vec<i32>,

    dealer_position: i32,
    small_blind_position: i32,
    big_blind_position: i32,
    action_on: i32,
    last_aggressor: i32,

    small_blind: i64,
    big_blind: i64,
    current_bet: i64,
    min_raise: i64,
    pot_total: i64,

    small_blind_posted: bool,
    big_blind_posted: bool,
}

/// The PM's aggregate state (rebuilt from its own events).
/// For simplicity, we're keeping the HandProcess inline.
#[derive(Clone, Default)]
struct PMState {
    process: Option<HandProcess>,
}

fn rebuild_state(_events: &EventBook) -> PMState {
    // In a full implementation, we'd rebuild the PM's state from its own events.
    // For now, start fresh each time (stateless-ish behavior).
    PMState::default()
}

fn prepare_hand_started(_trigger: &EventBook, _state: &PMState, event_any: &Any) -> Vec<Cover> {
    if let Ok(event) = HandStarted::decode(event_any.value.as_slice()) {
        vec![Cover {
            domain: "hand".to_string(),
            root: Some(Uuid { value: event.hand_root }),
            ..Default::default()
        }]
    } else {
        vec![]
    }
}

fn handle_hand_started(
    _trigger: &EventBook,
    _state: &PMState,
    event_any: &Any,
    _destinations: &[EventBook],
) -> CommandResult<ProcessManagerResponse> {
    let _event: HandStarted = HandStarted::decode(event_any.value.as_slice())
        .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;

    // Initialize hand process (not persisted in this simplified version)
    // The saga-table-hand will send DealCards, so we don't emit commands here.

    Ok(ProcessManagerResponse::default())
}

fn handle_cards_dealt(
    _trigger: &EventBook,
    _state: &PMState,
    event_any: &Any,
    _destinations: &[EventBook],
) -> CommandResult<ProcessManagerResponse> {
    let _event: CardsDealt = CardsDealt::decode(event_any.value.as_slice())
        .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;

    // Post small blind command
    // In a real implementation, we'd track state to know which blind to post.
    // For now, we assume the hand aggregate tracks this.

    Ok(ProcessManagerResponse::default())
}

fn handle_blind_posted(
    _trigger: &EventBook,
    _state: &PMState,
    event_any: &Any,
    _destinations: &[EventBook],
) -> CommandResult<ProcessManagerResponse> {
    let _event: BlindPosted = BlindPosted::decode(event_any.value.as_slice())
        .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;

    // In a full implementation, we'd check if both blinds are posted
    // and then start the betting round.

    Ok(ProcessManagerResponse::default())
}

fn handle_action_taken(
    _trigger: &EventBook,
    _state: &PMState,
    event_any: &Any,
    _destinations: &[EventBook],
) -> CommandResult<ProcessManagerResponse> {
    let _event: ActionTaken = ActionTaken::decode(event_any.value.as_slice())
        .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;

    // In a full implementation, we'd check if betting is complete
    // and advance to the next phase.

    Ok(ProcessManagerResponse::default())
}

fn handle_community_dealt(
    _trigger: &EventBook,
    _state: &PMState,
    event_any: &Any,
    _destinations: &[EventBook],
) -> CommandResult<ProcessManagerResponse> {
    let _event: CommunityCardsDealt = CommunityCardsDealt::decode(event_any.value.as_slice())
        .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;

    // Start new betting round after community cards.

    Ok(ProcessManagerResponse::default())
}

fn handle_pot_awarded(
    _trigger: &EventBook,
    _state: &PMState,
    event_any: &Any,
    _destinations: &[EventBook],
) -> CommandResult<ProcessManagerResponse> {
    let _event: PotAwarded = PotAwarded::decode(event_any.value.as_slice())
        .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;

    // Hand is complete. Clean up.

    Ok(ProcessManagerResponse::default())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = ProcessManagerRouter::new("hand-flow", "hand-flow", rebuild_state)
        .subscribes("table")
        .subscribes("hand")
        .sends("hand", "PostBlind")
        .sends("hand", "DealCommunityCards")
        .sends("hand", "AwardPot")
        .prepare("HandStarted", prepare_hand_started)
        .on("HandStarted", handle_hand_started)
        .on("CardsDealt", handle_cards_dealt)
        .on("BlindPosted", handle_blind_posted)
        .on("ActionTaken", handle_action_taken)
        .on("CommunityCardsDealt", handle_community_dealt)
        .on("PotAwarded", handle_pot_awarded);

    run_process_manager_server("hand-flow", 50091, router)
        .await
        .expect("Server failed");
}
