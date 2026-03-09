//! Hand Flow Process Manager - orchestrates poker hand phases across domains.
//!
//! This PM coordinates the workflow between table and hand domains,
//! tracking phase transitions as the hand progresses.
//!
//! Unlike sagas, a PM:
//! - Has its own persistent state (via correlation ID as aggregate root)
//! - Receives events from multiple domains
//! - Can make decisions based on accumulated state

use angzarr_client::proto::examples::{
    ActionTaken, BlindPosted, CardsDealt, CommunityCardsDealt, HandComplete, HandStarted,
    PotAwarded,
};
use angzarr_client::proto::{Cover, EventBook, Uuid};
use angzarr_client::{
    run_process_manager_server, CommandRejectedError, CommandResult, ProcessManagerDomainHandler,
    ProcessManagerResponse, ProcessManagerRouter, UnpackAny,
};
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// docs:start:pm_state
// docs:start:pm_state_oo
#[derive(Default, Clone)]
pub struct HandFlowState {
    hand_root: Vec<u8>,
    hand_number: i64,
    phase: HandPhase,
    blinds_posted: u32,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum HandPhase {
    #[default]
    AwaitingDeal,
    Dealing,
    Blinds,
    Betting,
    Complete,
}
// docs:end:pm_state_oo
// docs:end:pm_state

// docs:start:pm_handler
/// Process manager handler for hand flow orchestration.
///
/// Listens to events from both table and hand domains to coordinate
/// the poker hand lifecycle.
struct HandFlowPmHandler;

impl ProcessManagerDomainHandler<HandFlowState> for HandFlowPmHandler {
    fn event_types(&self) -> Vec<String> {
        vec![
            "HandStarted".into(),
            "CardsDealt".into(),
            "BlindPosted".into(),
            "ActionTaken".into(),
            "CommunityCardsDealt".into(),
            "PotAwarded".into(),
            "HandComplete".into(),
        ]
    }

    fn prepare(
        &self,
        _trigger: &EventBook,
        _state: &HandFlowState,
        event: &Any,
    ) -> Vec<Cover> {
        // Declare destinations needed based on the triggering event
        if event.type_url.ends_with("HandStarted") {
            if let Ok(evt) = event.unpack::<HandStarted>() {
                // We'll need access to the hand aggregate
                return vec![Cover {
                    domain: "hand".to_string(),
                    root: Some(Uuid {
                        value: evt.hand_root,
                    }),
                    ..Default::default()
                }];
            }
        }

        vec![]
    }

    fn handle(
        &self,
        _trigger: &EventBook,
        state: &HandFlowState,
        event: &Any,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        // Clone state for mutation (PM state is rebuilt from events, not mutated in-place)
        let mut local_state = state.clone();
        let type_url = &event.type_url;

        if type_url.ends_with("HandStarted") {
            return self.handle_hand_started(&mut local_state, event);
        } else if type_url.ends_with("CardsDealt") {
            return self.handle_cards_dealt(&mut local_state, event);
        } else if type_url.ends_with("BlindPosted") {
            return self.handle_blind_posted(&mut local_state, event);
        } else if type_url.ends_with("ActionTaken") {
            return self.handle_action_taken(&mut local_state, event);
        } else if type_url.ends_with("CommunityCardsDealt") {
            return self.handle_community_dealt(&mut local_state, event);
        } else if type_url.ends_with("PotAwarded") {
            return self.handle_pot_awarded(&mut local_state, event);
        } else if type_url.ends_with("HandComplete") {
            return self.handle_hand_complete(&mut local_state, event);
        }

        Ok(ProcessManagerResponse::default())
    }
}

impl HandFlowPmHandler {
    /// Handle HandStarted event from table domain.
    ///
    /// Initializes the PM state for tracking this hand.
    /// The saga-table-hand handles sending DealCards, so we don't emit commands here.
    fn handle_hand_started(
        &self,
        state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let event: HandStarted = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandStarted: {}", e)))?;

        state.hand_root = event.hand_root;
        state.hand_number = event.hand_number;
        state.phase = HandPhase::Dealing;

        // No commands - saga-table-hand sends DealCards
        Ok(ProcessManagerResponse::default())
    }

    /// Handle CardsDealt event from hand domain.
    ///
    /// Cards have been dealt, waiting for blinds.
    fn handle_cards_dealt(
        &self,
        state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let _event: CardsDealt = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode CardsDealt: {}", e)))?;

        state.phase = HandPhase::Blinds;

        // Blinds are posted by players/coordinator, not by PM
        Ok(ProcessManagerResponse::default())
    }

    /// Handle BlindPosted event from hand domain.
    fn handle_blind_posted(
        &self,
        state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let _event: BlindPosted = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode BlindPosted: {}", e)))?;

        state.blinds_posted += 1;

        // In a full implementation, check if all blinds posted then start betting
        if state.blinds_posted >= 2 {
            state.phase = HandPhase::Betting;
        }

        Ok(ProcessManagerResponse::default())
    }

    /// Handle ActionTaken event from hand domain.
    fn handle_action_taken(
        &self,
        state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let _event: ActionTaken = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode ActionTaken: {}", e)))?;

        // In a full implementation, track betting round progress
        // and advance phases when rounds complete
        let _ = state; // State tracking would go here

        Ok(ProcessManagerResponse::default())
    }

    /// Handle CommunityCardsDealt event from hand domain.
    fn handle_community_dealt(
        &self,
        _state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let _event: CommunityCardsDealt = event_any.unpack().map_err(|e| {
            CommandRejectedError::new(format!("Failed to decode CommunityCardsDealt: {}", e))
        })?;

        // New betting round starts after community cards
        Ok(ProcessManagerResponse::default())
    }

    /// Handle PotAwarded event from hand domain.
    fn handle_pot_awarded(
        &self,
        state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let _event: PotAwarded = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode PotAwarded: {}", e)))?;

        state.phase = HandPhase::Complete;

        Ok(ProcessManagerResponse::default())
    }

    /// Handle HandComplete event from hand domain.
    fn handle_hand_complete(
        &self,
        state: &mut HandFlowState,
        event_any: &Any,
    ) -> CommandResult<ProcessManagerResponse> {
        let _event: HandComplete = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandComplete: {}", e)))?;

        state.phase = HandPhase::Complete;

        // The saga-hand-table handles sending EndHand to table domain
        Ok(ProcessManagerResponse::default())
    }
}
// docs:end:pm_handler

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = ProcessManagerRouter::new("pmg-hand-flow", "pmg-hand-flow", |_| {
        HandFlowState::default()
    })
    .domain("table", HandFlowPmHandler)
    .domain("hand", HandFlowPmHandler);

    run_process_manager_server("pmg-hand-flow", 50391, router)
        .await
        .expect("Process manager failed");
}
