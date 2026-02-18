//! Saga: Table → Player
//!
//! Reacts to HandEnded events from Table domain.
//! Sends ReleaseFunds commands to Player domain.

use angzarr_client::proto::examples::{HandEnded, ReleaseFunds};
use angzarr_client::proto::{CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, EventRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Prepare handler: return destination covers for all players in StackChanges.
fn prepare_hand_ended(_source: &EventBook, event_any: &Any) -> Vec<Cover> {
    if let Ok(event) = HandEnded::decode(event_any.value.as_slice()) {
        event
            .stack_changes
            .keys()
            .filter_map(|player_hex| {
                hex::decode(player_hex).ok().map(|player_root| Cover {
                    domain: "player".to_string(),
                    root: Some(Uuid { value: player_root }),
                    ..Default::default()
                })
            })
            .collect()
    } else {
        vec![]
    }
}

/// Execute handler: translate HandEnded → ReleaseFunds for each player.
fn handle_hand_ended(
    source: &EventBook,
    event_any: &Any,
    destinations: &[EventBook],
) -> CommandResult<Vec<CommandBook>> {
    let event: HandEnded = event_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandEnded: {}", e)))?;

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

    // Create ReleaseFunds commands for all players
    let commands: Vec<CommandBook> = event
        .stack_changes
        .keys()
        .filter_map(|player_hex| {
            let player_root = hex::decode(player_hex).ok()?;

            // Get sequence from destination state
            let dest_seq = dest_map
                .get(&player_root)
                .map(|eb| eb.next_sequence)
                .unwrap_or(0);

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
                pages: vec![CommandPage {
                    sequence: dest_seq,
                    command: Some(command_any),
                    ..Default::default()
                }],
                saga_origin: None,
            })
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

    let router = EventRouter::new("saga-table-player", "table")
        .sends("player", "ReleaseFunds")
        .prepare("HandEnded", prepare_hand_ended)
        .on_many("HandEnded", handle_hand_ended);

    run_saga_server("saga-table-player", 50013, router)
        .await
        .expect("Server failed");
}
