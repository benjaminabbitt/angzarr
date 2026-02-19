//! Saga: Table → Hand
//!
//! DOC: This file is referenced in docs/docs/examples/sagas.mdx
//!      Update documentation when making changes to saga patterns.
//!
//! Reacts to HandStarted events from Table domain.
//! Sends DealCards commands to Hand domain.

use angzarr_client::proto::examples::{DealCards, HandStarted, PlayerInHand};
use angzarr_client::proto::{command_page, CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{
    run_saga_server, CommandRejectedError, CommandResult, EventRouter, UnpackAny,
};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Prepare handler: return the destination cover to fetch (hand aggregate).
fn prepare_hand_started(_source: &EventBook, event_any: &Any) -> Vec<Cover> {
    if let Ok(event) = HandStarted::decode(event_any.value.as_slice()) {
        // The hand aggregate root is in the event
        vec![Cover {
            domain: "hand".to_string(),
            root: Some(Uuid { value: event.hand_root }),
            ..Default::default()
        }]
    } else {
        vec![]
    }
}

// docs:start:saga_handler
/// Execute handler: translate HandStarted → DealCards.
fn handle_hand_started(
    _source: &EventBook,
    event_any: &Any,
    destinations: &[EventBook],
) -> CommandResult<Option<CommandBook>> {
    let event: HandStarted = event_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode HandStarted: {}", e)))?;

    // Get the destination's next sequence
    let dest_seq = destinations
        .first()
        .map(|eb| eb.next_sequence)
        .unwrap_or(0);

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

    Ok(Some(CommandBook {
        cover: Some(Cover {
            domain: "hand".to_string(),
            root: Some(Uuid { value: event.hand_root }),
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
// docs:end:saga_handler

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // docs:start:event_router
    let router = EventRouter::new("saga-table-hand")
        .domain("table")
        .prepare("HandStarted", prepare_hand_started)
        .on("HandStarted", handle_hand_started);
    // docs:end:event_router

    run_saga_server("saga-table-hand", 50011, router)
        .await
        .expect("Server failed");
}
