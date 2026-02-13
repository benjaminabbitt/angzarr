//! Saga: Hand → Player
//!
//! Reacts to PotAwarded events from Hand domain.
//! Sends DepositFunds commands to Player domain.

use angzarr_client::proto::examples::{Currency, DepositFunds, PotAwarded};
use angzarr_client::proto::{CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, EventRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Prepare handler: return destination covers for all winners.
fn prepare_pot_awarded(_source: &EventBook, event_any: &Any) -> Vec<Cover> {
    if let Ok(event) = PotAwarded::decode(event_any.value.as_slice()) {
        event
            .winners
            .iter()
            .map(|winner| Cover {
                domain: "player".to_string(),
                root: Some(Uuid { value: winner.player_root.clone() }),
                ..Default::default()
            })
            .collect()
    } else {
        vec![]
    }
}

/// Execute handler: translate PotAwarded → DepositFunds for each winner.
fn handle_pot_awarded(
    source: &EventBook,
    event_any: &Any,
    destinations: &[EventBook],
) -> CommandResult<Vec<CommandBook>> {
    let event: PotAwarded = event_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode PotAwarded: {}", e)))?;

    // Get correlation ID from source
    let correlation_id = source
        .cover
        .as_ref()
        .map(|c| c.correlation_id.clone())
        .unwrap_or_default();

    // Build a map from player root to destination for sequence lookup
    let dest_map: std::collections::HashMap<Vec<u8>, &EventBook> = destinations
        .iter()
        .filter_map(|eb| {
            eb.cover
                .as_ref()
                .and_then(|c| c.root.as_ref())
                .map(|u| (u.value.clone(), eb))
        })
        .collect();

    // Create DepositFunds commands for each winner
    let commands: Vec<CommandBook> = event
        .winners
        .iter()
        .map(|winner| {
            // Get sequence from destination state
            let dest_seq = dest_map
                .get(&winner.player_root)
                .map(|eb| eb.next_sequence)
                .unwrap_or(0);

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
                pages: vec![CommandPage {
                    sequence: dest_seq,
                    command: Some(command_any),
                }],
                saga_origin: None,
            }
        })
        .collect();

    Ok(commands)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = EventRouter::new("saga-hand-player", "hand")
        .sends("player", "DepositFunds")
        .prepare("PotAwarded", prepare_pot_awarded)
        .on_many("PotAwarded", handle_pot_awarded);

    run_saga_server("saga-hand-player", 50014, router)
        .await
        .expect("Server failed");
}
