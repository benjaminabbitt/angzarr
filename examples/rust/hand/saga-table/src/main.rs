//! Saga: Hand → Table
//!
//! Reacts to HandComplete events from Hand domain.
//! Sends EndHand commands to Table domain.
//!
//! This saga is a pure translator - it receives source events and produces
//! commands without knowing destination state. The framework handles:
//! - Sequence assignment (via angzarr_deferred)
//! - Idempotency checking
//! - Delivery retry on sequence conflicts

use angzarr_client::proto::examples::{EndHand, HandComplete, PotResult};
use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, SagaDomainHandler, SagaHandlerResponse,
    SagaRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Saga handler for Hand → Table domain translation.
struct HandTableSagaHandler;

impl SagaDomainHandler for HandTableSagaHandler {
    fn event_types(&self) -> Vec<String> {
        vec!["HandComplete".into()]
    }

    fn handle(&self, source: &EventBook, event: &Any) -> CommandResult<SagaHandlerResponse> {
        if event.type_url.ends_with("HandComplete") {
            return Self::handle_hand_complete(source, event);
        }
        Ok(SagaHandlerResponse::default())
    }
}

impl HandTableSagaHandler {
    /// Translate HandComplete → EndHand.
    ///
    /// Commands use deferred sequences - framework assigns on delivery.
    fn handle_hand_complete(
        source: &EventBook,
        event_any: &Any,
    ) -> CommandResult<SagaHandlerResponse> {
        let event: HandComplete = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandComplete: {}", e)))?;

        // Get hand_root from source cover
        let hand_root = source
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|u| u.value.clone())
            .unwrap_or_default();

        // Convert PotWinner to PotResult
        let results: Vec<PotResult> = event
            .winners
            .iter()
            .map(|winner| PotResult {
                winner_root: winner.player_root.clone(),
                amount: winner.amount,
                pot_type: winner.pot_type.clone(),
                winning_hand: winner.winning_hand.clone(),
            })
            .collect();

        // Build EndHand command
        let end_hand = EndHand {
            hand_root,
            results,
        };

        let command_any = Any {
            type_url: "type.googleapis.com/examples.EndHand".to_string(),
            value: end_hand.encode_to_vec(),
        };

        Ok(SagaHandlerResponse {
            commands: vec![CommandBook {
                cover: Some(Cover {
                    domain: "table".to_string(),
                    root: Some(Uuid { value: event.table_root }),
                    ..Default::default()
                }),
                // Framework will stamp angzarr_deferred with source info
                // and assign sequence on delivery
                pages: vec![CommandPage {
                    payload: Some(command_page::Payload::Command(command_any)),
                    ..Default::default()
                }],
            }],
            events: vec![],
        })
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = SagaRouter::new("saga-hand-table", "hand", HandTableSagaHandler);

    run_saga_server("saga-hand-table", 50012, router)
        .await
        .expect("Server failed");
}
