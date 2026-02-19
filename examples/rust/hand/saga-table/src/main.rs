//! Saga: Hand → Table
//!
//! Reacts to HandComplete events from Hand domain.
//! Sends EndHand commands to Table domain.

use angzarr_client::proto::examples::{EndHand, HandComplete, PotResult};
use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, EventRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Prepare handler: return the destination cover to fetch (table aggregate).
fn prepare_hand_complete(_source: &EventBook, event_any: &Any) -> Vec<Cover> {
    if let Ok(event) = HandComplete::decode(event_any.value.as_slice()) {
        vec![Cover {
            domain: "table".to_string(),
            root: Some(Uuid { value: event.table_root }),
            ..Default::default()
        }]
    } else {
        vec![]
    }
}

/// Execute handler: translate HandComplete → EndHand.
fn handle_hand_complete(
    source: &EventBook,
    event_any: &Any,
    destinations: &[EventBook],
) -> CommandResult<Option<CommandBook>> {
    let event: HandComplete = event_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandComplete: {}", e)))?;

    // Get the destination's next sequence
    let dest_seq = destinations
        .first()
        .map(|eb| eb.next_sequence)
        .unwrap_or(0);

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

    Ok(Some(CommandBook {
        cover: Some(Cover {
            domain: "table".to_string(),
            root: Some(Uuid { value: event.table_root }),
            ..Default::default()
        }),
        pages: vec![CommandPage {
            sequence: dest_seq,
            payload: Some(command_page::Payload::Command(command_any)),
            ..Default::default()
        }],
        saga_origin: None,
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let router = EventRouter::new("saga-hand-table")
        .domain("hand")
        .prepare("HandComplete", prepare_hand_complete)
        .on("HandComplete", handle_hand_complete);

    run_saga_server("saga-hand-table", 50012, router)
        .await
        .expect("Server failed");
}
