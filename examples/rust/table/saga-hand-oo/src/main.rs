//! Saga: Table → Hand (OO Pattern)
//!
//! DOC: This file is referenced in docs/docs/examples/sagas.mdx
//!      Update documentation when making changes to saga patterns.
//!
//! Reacts to HandStarted events from Table domain.
//! Sends DealCards commands to Hand domain.
//!
//! This example demonstrates the OO pattern using:
//! - `#[saga(name = "...", input = "...", output = "...")]` on impl blocks
//! - `#[prepares(EventType)]` on prepare methods
//! - `#[reacts_to(EventType)]` on handler methods

use angzarr_client::proto::examples::{DealCards, HandStarted, PlayerInHand};
use angzarr_client::proto::{CommandBook, CommandPage, Cover, EventBook, Uuid};
use angzarr_client::{run_saga_server, CommandResult};
#[allow(unused_imports)]
use angzarr_macros::{prepares, reacts_to, saga};
use prost::Message;
use prost_types::Any;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Table→Hand saga using OO-style annotations.
pub struct TableHandSaga;

#[saga(name = "saga-table-hand", input = "table", output = "hand")]
impl TableHandSaga {
    /// Prepare handler: declare destination cover to fetch.
    #[prepares(HandStarted)]
    fn prepare_hand_started(&self, event: &HandStarted) -> Vec<Cover> {
        vec![Cover {
            domain: "hand".to_string(),
            root: Some(Uuid {
                value: event.hand_root.clone(),
            }),
            ..Default::default()
        }]
    }

    /// Execute handler: translate HandStarted → DealCards.
    #[reacts_to(HandStarted)]
    fn handle_hand_started(
        &self,
        event: HandStarted,
        destinations: &[EventBook],
    ) -> CommandResult<Vec<CommandBook>> {
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
            table_root: event.hand_root.clone(),
            hand_number: event.hand_number,
            game_variant: event.game_variant,
            players,
            dealer_position: event.dealer_position,
            small_blind: event.small_blind,
            big_blind: event.big_blind,
            deck_seed: vec![],
        };

        let command_any = Any {
            type_url: "type.googleapis.com/examples.DealCards".to_string(),
            value: deal_cards.encode_to_vec(),
        };

        Ok(vec![CommandBook {
            cover: Some(Cover {
                domain: "hand".to_string(),
                root: Some(Uuid {
                    value: event.hand_root,
                }),
                ..Default::default()
            }),
            pages: vec![CommandPage {
                sequence: dest_seq,
                payload: Some(angzarr_client::proto::command_page::Payload::Command(command_any)),
                ..Default::default()
            }],
            saga_origin: None,
        }])
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let saga = TableHandSaga;
    let router = saga.into_router();

    println!("Starting Table→Hand saga (OO pattern)");
    println!("Name: {}", router.name());

    run_saga_server("saga-table-hand", 50021, router)
        .await
        .expect("Server failed");
}
