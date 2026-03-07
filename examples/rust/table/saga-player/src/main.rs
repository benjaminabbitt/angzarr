//! Saga: Table → Player
//!
//! Reacts to HandEnded events from Table domain.
//! Sends ReleaseFunds commands to Player domain.
//!
//! This saga is a pure translator - it receives source events and produces
//! commands without knowing destination state. The framework handles:
//! - Sequence assignment (via angzarr_deferred)
//! - Idempotency checking
//! - Delivery retry on sequence conflicts

use angzarr_client::proto::examples::{HandEnded, ReleaseFunds};
use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, SagaDomainHandler, SagaHandlerResponse,
    SagaRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Saga handler for Table → Player domain translation.
struct TablePlayerSagaHandler;

impl SagaDomainHandler for TablePlayerSagaHandler {
    fn event_types(&self) -> Vec<String> {
        vec!["HandEnded".into()]
    }

    fn handle(&self, source: &EventBook, event: &Any) -> CommandResult<SagaHandlerResponse> {
        if event.type_url.ends_with("HandEnded") {
            return Self::handle_hand_ended(source, event);
        }
        Ok(SagaHandlerResponse::default())
    }
}

impl TablePlayerSagaHandler {
    /// Translate HandEnded → ReleaseFunds for each player.
    ///
    /// Commands use deferred sequences - framework assigns on delivery.
    fn handle_hand_ended(
        source: &EventBook,
        event_any: &Any,
    ) -> CommandResult<SagaHandlerResponse> {
        let event: HandEnded = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandEnded: {}", e)))?;

        // Get correlation ID from source
        let correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        // Create ReleaseFunds commands for all players
        // Note: No explicit sequence - framework stamps via angzarr_deferred
        let commands: Vec<CommandBook> = event
            .stack_changes
            .keys()
            .filter_map(|player_hex| {
                let player_root = hex::decode(player_hex).ok()?;

                let release_funds = ReleaseFunds {
                    table_root: event.hand_root.clone(),
                };

                let command_any = Any {
                    type_url: "type.googleapis.com/examples.ReleaseFunds".to_string(),
                    value: release_funds.encode_to_vec(),
                };

                Some(CommandBook {
                    cover: Some(Cover {
                        domain: "player".to_string(),
                        root: Some(Uuid { value: player_root }),
                        correlation_id: correlation_id.clone(),
                        ..Default::default()
                    }),
                    // Framework will stamp angzarr_deferred with source info
                    // and assign sequence on delivery
                    pages: vec![CommandPage {
                        payload: Some(command_page::Payload::Command(command_any)),
                        ..Default::default()
                    }],
                })
            })
            .collect();

        Ok(SagaHandlerResponse {
            commands,
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

    let router = SagaRouter::new("saga-table-player", "table", TablePlayerSagaHandler);

    run_saga_server("saga-table-player", 50013, router)
        .await
        .expect("Server failed");
}
