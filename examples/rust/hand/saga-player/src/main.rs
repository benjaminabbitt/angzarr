//! Saga: Hand → Player
//!
//! Reacts to PotAwarded events from Hand domain.
//! Sends DepositFunds commands to Player domain.
//!
//! This saga is a pure translator - it receives source events and produces
//! commands without knowing destination state. The framework handles:
//! - Sequence assignment (via angzarr_deferred)
//! - Idempotency checking
//! - Delivery retry on sequence conflicts

use angzarr_client::proto::examples::{Currency, DepositFunds, PotAwarded};
use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, SagaDomainHandler, SagaHandlerResponse,
    SagaRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Saga handler for Hand → Player domain translation.
struct HandPlayerSagaHandler;

impl SagaDomainHandler for HandPlayerSagaHandler {
    fn event_types(&self) -> Vec<String> {
        vec!["PotAwarded".into()]
    }

    fn handle(&self, source: &EventBook, event: &Any) -> CommandResult<SagaHandlerResponse> {
        if event.type_url.ends_with("PotAwarded") {
            return Self::handle_pot_awarded(source, event);
        }
        Ok(SagaHandlerResponse::default())
    }
}

impl HandPlayerSagaHandler {
    /// Translate PotAwarded → DepositFunds for each winner.
    ///
    /// Commands use deferred sequences - framework assigns on delivery.
    fn handle_pot_awarded(
        source: &EventBook,
        event_any: &Any,
    ) -> CommandResult<SagaHandlerResponse> {
        let event: PotAwarded = event_any
            .unpack()
            .map_err(|e| CommandRejectedError::new(format!("Failed to decode PotAwarded: {}", e)))?;

        // Get correlation ID from source
        let correlation_id = source
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        // Create DepositFunds commands for each winner
        // Note: No explicit sequence - framework stamps via angzarr_deferred
        let commands: Vec<CommandBook> = event
            .winners
            .iter()
            .map(|winner| {
                let deposit_funds = DepositFunds {
                    amount: Some(Currency {
                        amount: winner.amount,
                        currency_code: String::new(),
                    }),
                };

                let command_any = Any {
                    type_url: "type.googleapis.com/examples.DepositFunds".to_string(),
                    value: deposit_funds.encode_to_vec(),
                };

                CommandBook {
                    cover: Some(Cover {
                        domain: "player".to_string(),
                        root: Some(Uuid { value: winner.player_root.clone() }),
                        correlation_id: correlation_id.clone(),
                        ..Default::default()
                    }),
                    // Framework will stamp angzarr_deferred with source info
                    // and assign sequence on delivery
                    pages: vec![CommandPage {
                        payload: Some(command_page::Payload::Command(command_any)),
                        ..Default::default()
                    }],
                }
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

    let router = SagaRouter::new("saga-hand-player", "hand", HandPlayerSagaHandler);

    run_saga_server("saga-hand-player", 50014, router)
        .await
        .expect("Server failed");
}
