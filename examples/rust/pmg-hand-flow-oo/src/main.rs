//! Hand Flow Process Manager - OO Pattern
//!
//! Orchestrates poker hand phases across domains using the OO pattern with
//! `#[process_manager]`, `#[handles]`, and `#[prepares]` macros.
//!
//! Compare with the functional pattern in pmg-hand-flow/.

use angzarr_client::proto::examples::{
    CardsDealt, DealCards, EndHand, HandComplete, HandStarted, PostBlinds,
};
use angzarr_client::proto::{CommandBook, Cover, EventBook, Uuid};
use angzarr_client::{run_process_manager_server, CommandResult, ProcessManagerResponse};
use angzarr_macros::{handles, prepares, process_manager};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// docs:start:pm_state_oo
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
// docs:end:pm_state_oo

// docs:start:pm_handler_oo
pub struct HandFlowPM;

#[process_manager(
    name = "pmg-hand-flow",
    domain = "pmg-hand-flow",
    state = HandFlowState,
    inputs = ["table", "hand"]
)]
impl HandFlowPM {
    #[prepares(HandStarted)]
    fn prepare_hand_started(
        &self,
        _trigger: &EventBook,
        _state: &HandFlowState,
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
        _state: &HandFlowState,
        event: HandStarted,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        let cmd = DealCards {
            hand_id: event.hand_id.clone(),
            player_count: event.player_count,
            ..Default::default()
        };

        Ok(ProcessManagerResponse {
            commands: vec![build_command("hand", &event.hand_root, cmd)],
            ..Default::default()
        })
    }

    #[handles(CardsDealt)]
    fn handle_cards_dealt(
        &self,
        _trigger: &EventBook,
        _state: &HandFlowState,
        event: CardsDealt,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        let cmd = PostBlinds {
            hand_id: event.hand_id.clone(),
            ..Default::default()
        };

        Ok(ProcessManagerResponse {
            commands: vec![build_command("hand", &event.hand_root, cmd)],
            ..Default::default()
        })
    }

    #[handles(HandComplete)]
    fn handle_hand_complete(
        &self,
        _trigger: &EventBook,
        _state: &HandFlowState,
        event: HandComplete,
        _destinations: &[EventBook],
    ) -> CommandResult<ProcessManagerResponse> {
        let cmd = EndHand {
            hand_id: event.hand_id.clone(),
            winner_id: event.winner_id.clone(),
            ..Default::default()
        };

        Ok(ProcessManagerResponse {
            commands: vec![build_command("table", &event.table_root, cmd)],
            ..Default::default()
        })
    }
}
// docs:end:pm_handler_oo

fn build_command<M: prost::Message>(domain: &str, root: &[u8], msg: M) -> CommandBook {
    use prost_types::Any;

    let type_url = format!(
        "type.googleapis.com/examples.{}",
        std::any::type_name::<M>().rsplit("::").next().unwrap_or("")
    );

    let mut value = Vec::new();
    msg.encode(&mut value).unwrap();

    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(Uuid {
                value: root.to_vec(),
            }),
            ..Default::default()
        }),
        pages: vec![angzarr_client::proto::CommandPage {
            sequence_type: None,
            payload: Some(angzarr_client::proto::command_page::Payload::Command(Any {
                type_url,
                value,
            })),
        }],
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let pm = HandFlowPM;
    let router = pm.into_router();

    println!("Starting Hand Flow process manager (OO pattern)");

    run_process_manager_server("pmg-hand-flow", 50392, router)
        .await
        .expect("Server failed");
}
