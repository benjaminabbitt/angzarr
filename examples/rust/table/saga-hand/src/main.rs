//! Saga: Table → Hand
//!
//! DOC: This file is referenced in docs/docs/examples/sagas.mdx
//!      Update documentation when making changes to saga patterns.
//!
//! Reacts to HandStarted events from Table domain.
//! Sends DealCards commands to Hand domain.
//!
//! This saga is a pure translator - it receives source events and produces
//! commands without knowing destination state. The framework handles:
//! - Sequence assignment (via angzarr_deferred)
//! - Idempotency checking
//! - Delivery retry on sequence conflicts

use angzarr_client::proto::examples::{DealCards, HandStarted, PlayerInHand};
use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, SagaDomainHandler, SagaHandlerResponse,
    SagaRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// docs:start:saga_handler
// docs:start:saga_oo
/// Saga handler for Table → Hand domain translation.
struct TableHandSagaHandler;

impl SagaDomainHandler for TableHandSagaHandler {
    fn event_types(&self) -> Vec<String> {
        vec!["HandStarted".into()]
    }

    fn handle(&self, source: &EventBook, event: &Any) -> CommandResult<SagaHandlerResponse> {
        if event.type_url.ends_with("HandStarted") {
            return Self::handle_hand_started(source, event);
        }
        Ok(SagaHandlerResponse::default())
    }
}

impl TableHandSagaHandler {
    /// Translate HandStarted → DealCards.
    ///
    /// Commands use deferred sequences - framework assigns on delivery.
    fn handle_hand_started(
        _source: &EventBook,
        event_any: &Any,
    ) -> CommandResult<SagaHandlerResponse> {
        let event: HandStarted = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandStarted: {}", e)))?;

        // Convert SeatSnapshot to PlayerInHand
        let players: Vec<PlayerInHand> = event
            .active_players
            .iter()
            .map(|seat| PlayerInHand {
                player_root: seat.player_root.clone(),
                position: seat.position,
                stack: seat.stack,
            })
            .collect();

        // Build DealCards command
        let deal_cards = DealCards {
            table_root: event.hand_root.clone(), // The hand_root becomes the table_root reference
            hand_number: event.hand_number,
            game_variant: event.game_variant,
            players,
            dealer_position: event.dealer_position,
            small_blind: event.small_blind,
            big_blind: event.big_blind,
            deck_seed: vec![], // Let the aggregate generate a random seed
        };

        let command_any = Any {
            type_url: "type.googleapis.com/examples.DealCards".to_string(),
            value: deal_cards.encode_to_vec(),
        };

        Ok(SagaHandlerResponse {
            commands: vec![CommandBook {
                cover: Some(Cover {
                    domain: "hand".to_string(),
                    root: Some(Uuid { value: event.hand_root }),
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
// docs:end:saga_oo
// docs:end:saga_handler

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // docs:start:event_router
    let router = SagaRouter::new("saga-table-hand", "table", TableHandSagaHandler);
    // docs:end:event_router

    run_saga_server("saga-table-hand", 50011, router)
        .await
        .expect("Server failed");
}
