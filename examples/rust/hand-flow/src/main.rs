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
    run_process_manager_server, CommandResult, ProcessManagerDomainHandler, ProcessManagerResponse,
    ProcessManagerRouter,
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
pub struct PMState {
    process: Option<HandProcess>,
}

fn rebuild_state(_events: &EventBook) -> PMState {
    // In a full implementation, we'd rebuild the PM's state from its own events.
    // For now, start fresh each time (stateless-ish behavior).
    PMState::default()
}

/// Handler for table domain events in the hand-flow PM.
struct TableDomainHandler;

impl ProcessManagerDomainHandler<PMState> for TableDomainHandler {
    fn event_types(&self) -> Vec<String> {
        vec!["HandStarted".into()]
    }

    fn prepare(&self, _trigger: &EventBook, _state: &PMState, event: &Any) -> Vec<Cover> {
        if event.type_url.ends_with("HandStarted") {
            if let Ok(evt) = HandStarted::decode(event.value.as_slice()) {
                return vec![Cover {
                    domain: "hand".to_string(),
                    root: Some(Uuid { value: evt.hand_root }),
                    ..Default::default()
                }];
            }
        }
        vec![]
    }

    fn handle(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        event: &Any,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        if event.type_url.ends_with("HandStarted") {
            let _event: HandStarted = HandStarted::decode(event.value.as_slice())
                .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;
            // Initialize hand process (not persisted in this simplified version)
            // The saga-table-hand will send DealCards, so we don't emit commands here.
        }
        Ok(ProcessManagerResponse::default())
    }
}

/// Handler for hand domain events in the hand-flow PM.
struct HandDomainHandler;

impl ProcessManagerDomainHandler<PMState> for HandDomainHandler {
    fn event_types(&self) -> Vec<String> {
        vec![
            "CardsDealt".into(),
            "BlindPosted".into(),
            "ActionTaken".into(),
            "CommunityCardsDealt".into(),
            "PotAwarded".into(),
        ]
    }

    fn prepare(&self, trigger: &EventBook, _state: &PMState, _event: &Any) -> Vec<Cover> {
        // Hand domain events - use trigger's root directly for sequence lookup
        if let Some(cover) = &trigger.cover {
            if let Some(root) = &cover.root {
                return vec![Cover {
                    domain: "hand".to_string(),
                    root: Some(root.clone()),
                    ..Default::default()
                }];
            }
        }
        vec![]
    }

    fn handle(
        &self,
        _trigger: &EventBook,
        _state: &PMState,
        event: &Any,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        let type_url = &event.type_url;

        if type_url.ends_with("CardsDealt") {
            let _evt: CardsDealt = CardsDealt::decode(event.value.as_slice())
                .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;
            // Post small blind command
        } else if type_url.ends_with("BlindPosted") {
            let _evt: BlindPosted = BlindPosted::decode(event.value.as_slice())
                .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;
            // Check if both blinds are posted
        } else if type_url.ends_with("ActionTaken") {
            let _evt: ActionTaken = ActionTaken::decode(event.value.as_slice())
                .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;
            // Check if betting is complete
        } else if type_url.ends_with("CommunityCardsDealt") {
            let _evt: CommunityCardsDealt = CommunityCardsDealt::decode(event.value.as_slice())
                .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;
            // Start new betting round
        } else if type_url.ends_with("PotAwarded") {
            let _evt: PotAwarded = PotAwarded::decode(event.value.as_slice())
                .map_err(|e| angzarr_client::CommandRejectedError::new(e.to_string()))?;
            // Hand is complete
        }

        Ok(ProcessManagerResponse::default())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = ProcessManagerRouter::new("hand-flow", "hand-flow", rebuild_state)
        .domain("table", TableDomainHandler)
        .domain("hand", HandDomainHandler);

    run_process_manager_server("hand-flow", 50091, router)
        .await
        .expect("Server failed");
}
