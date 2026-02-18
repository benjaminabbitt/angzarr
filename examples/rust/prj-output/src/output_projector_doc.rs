//! Output projector examples for documentation.
//!
//! This file contains simplified examples used in the projector documentation,
//! demonstrating both OO-style and StateRouter patterns.

use std::collections::HashMap;

use angzarr_client::proto::examples::{CardsDealt, FundsDeposited, PlayerRegistered};
use angzarr_client::StateRouter;

// docs:start:projector_oo
pub struct OutputProjector {
    player_names: HashMap<String, String>,
}

impl OutputProjector {
    pub fn handle_player_registered(&mut self, event: &PlayerRegistered) {
        self.player_names.insert(event.player_id.clone(), event.display_name.clone());
        println!("[Player] {} registered", event.display_name);
    }

    pub fn handle_funds_deposited(&mut self, event: &FundsDeposited) {
        let name = self.player_names.get(&event.player_id)
            .map(|s| s.as_str())
            .unwrap_or(&event.player_id);
        let amount = event.amount.as_ref().map(|a| a.amount).unwrap_or(0);
        println!("[Player] {} deposited ${:.2}", name, amount as f64 / 100.0);
    }

    pub fn handle_cards_dealt(&mut self, event: &CardsDealt) {
        for player in &event.player_cards {
            let name = self.player_names.get(&player.player_id)
                .map(|s| s.as_str())
                .unwrap_or(&player.player_id);
            let cards = format_cards(&player.hole_cards);
            println!("[Hand] {} dealt {}", name, cards);
        }
    }
}
// docs:end:projector_oo

fn format_cards(cards: &[angzarr_client::proto::examples::Card]) -> String {
    cards.iter().map(|c| format!("{}{}", c.rank, c.suit)).collect::<Vec<_>>().join(" ")
}

// docs:start:state_router
fn build_router() -> StateRouter {
    StateRouter::new("prj-output")
        .subscribes("player", &["PlayerRegistered", "FundsDeposited"])
        .subscribes("hand", &["CardsDealt", "ActionTaken", "PotAwarded"])
        .on::<PlayerRegistered>(handle_player_registered)
        .on::<FundsDeposited>(handle_funds_deposited)
        .on::<CardsDealt>(handle_cards_dealt)
}

fn handle_player_registered(event: &PlayerRegistered, state: &mut ProjectorState) {
    state.player_names.insert(event.player_id.clone(), event.display_name.clone());
    println!("[Player] {} registered", event.display_name);
}

fn handle_funds_deposited(event: &FundsDeposited, state: &mut ProjectorState) {
    let name = state.player_names.get(&event.player_id)
        .map(|s| s.as_str())
        .unwrap_or(&event.player_id);
    println!("[Player] {} deposited", name);
}

fn handle_cards_dealt(event: &CardsDealt, state: &mut ProjectorState) {
    for player in &event.player_cards {
        let name = state.player_names.get(&player.player_id)
            .map(|s| s.as_str())
            .unwrap_or(&player.player_id);
        println!("[Hand] {} dealt cards", name);
    }
}

struct ProjectorState {
    player_names: HashMap<String, String>,
}
// docs:end:state_router
