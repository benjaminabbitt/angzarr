//! Hand Flow Process Manager - orchestrates poker hand phases across domains.
//!
//! This PM coordinates the workflow between table and hand domains,
//! tracking phase transitions and dispatching commands as the hand progresses.

use angzarr_client::proto::angzarr::CommandBook;
use angzarr_client::proto::examples::{
    CardsDealt, DealCards, EndHand, HandComplete, HandStarted, PostBlinds,
};
use angzarr_client::{build_command, ProcessManager};

// docs:start:pm_state
#[derive(Default)]
pub struct HandFlowState {
    hand_id: String,
    phase: HandPhase,
    player_count: u32,
}

#[derive(Default, PartialEq)]
pub enum HandPhase {
    #[default]
    AwaitingDeal,
    Dealing,
    Blinds,
    Betting,
    Complete,
}
// docs:end:pm_state

// docs:start:pm_handler
pub struct HandFlowPM;

impl HandFlowPM {
    pub fn handle_hand_started(
        &self,
        event: &HandStarted,
        state: &mut HandFlowState,
    ) -> Vec<CommandBook> {
        state.hand_id = event.hand_id.clone();
        state.phase = HandPhase::Dealing;
        state.player_count = event.player_count;

        vec![build_command("hand", DealCards {
            hand_id: event.hand_id.clone(),
            player_count: event.player_count,
        })]
    }

    pub fn handle_cards_dealt(
        &self,
        _event: &CardsDealt,
        state: &mut HandFlowState,
    ) -> Vec<CommandBook> {
        state.phase = HandPhase::Blinds;
        vec![build_command("hand", PostBlinds {
            hand_id: state.hand_id.clone(),
        })]
    }

    pub fn handle_hand_complete(
        &self,
        event: &HandComplete,
        state: &mut HandFlowState,
    ) -> Vec<CommandBook> {
        state.phase = HandPhase::Complete;
        vec![build_command("table", EndHand {
            hand_id: state.hand_id.clone(),
            winner_id: event.winner_id.clone(),
        })]
    }
}
// docs:end:pm_handler

#[tokio::main]
async fn main() {
    let pm = HandFlowPM;
    angzarr_client::run_process_manager("pmg-hand-flow", 50391, pm)
        .await
        .expect("Process manager failed");
}
