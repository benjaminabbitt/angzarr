//! Player aggregate state.

use std::collections::HashMap;

use angzarr_client::proto::examples::{PlayerState as ProtoPlayerState, PlayerType};
use angzarr_client::proto::EventBook;
use angzarr_client::UnpackAny;
use prost::Message;

/// Player aggregate state rebuilt from events.
#[derive(Debug, Default, Clone)]
pub struct PlayerState {
    pub player_id: String,
    pub display_name: String,
    pub email: String,
    pub player_type: PlayerType,
    pub ai_model_id: String,
    pub bankroll: i64,
    pub reserved_funds: i64,
    pub table_reservations: HashMap<String, i64>, // table_root_hex -> amount
    pub status: String,
}

impl PlayerState {
    /// Check if the player exists.
    pub fn exists(&self) -> bool {
        !self.player_id.is_empty()
    }

    /// Get available balance (bankroll - reserved).
    pub fn available_balance(&self) -> i64 {
        self.bankroll - self.reserved_funds
    }

    /// Check if this is an AI player.
    pub fn is_ai(&self) -> bool {
        self.player_type == PlayerType::Ai
    }
}

/// Rebuild player state from event history.
pub fn rebuild_state(event_book: &EventBook) -> PlayerState {
    let mut state = PlayerState::default();

    // Start from snapshot if available
    if let Some(snapshot) = &event_book.snapshot {
        if let Some(snapshot_any) = &snapshot.state {
            if let Ok(proto_state) = snapshot_any.unpack::<ProtoPlayerState>() {
                state = apply_snapshot(&proto_state);
            }
        }
    }

    // Apply events since snapshot
    for page in &event_book.pages {
        if let Some(event) = &page.event {
            apply_event(&mut state, event);
        }
    }

    state
}

fn apply_snapshot(snapshot: &ProtoPlayerState) -> PlayerState {
    let bankroll = snapshot.bankroll.as_ref().map(|c| c.amount).unwrap_or(0);
    let reserved_funds = snapshot
        .reserved_funds
        .as_ref()
        .map(|c| c.amount)
        .unwrap_or(0);

    PlayerState {
        player_id: snapshot.player_id.clone(),
        display_name: snapshot.display_name.clone(),
        email: snapshot.email.clone(),
        player_type: PlayerType::try_from(snapshot.player_type).unwrap_or_default(),
        ai_model_id: snapshot.ai_model_id.clone(),
        bankroll,
        reserved_funds,
        table_reservations: snapshot.table_reservations.clone(),
        status: snapshot.status.clone(),
    }
}

fn apply_event(state: &mut PlayerState, event_any: &prost_types::Any) {
    use angzarr_client::proto::examples::*;

    let type_url = &event_any.type_url;

    if type_url.ends_with("PlayerRegistered") {
        if let Ok(event) = PlayerRegistered::decode(event_any.value.as_slice()) {
            state.player_id = format!("player_{}", event.email);
            state.display_name = event.display_name;
            state.email = event.email;
            state.player_type = PlayerType::try_from(event.player_type).unwrap_or_default();
            state.ai_model_id = event.ai_model_id;
            state.status = "active".to_string();
            state.bankroll = 0;
            state.reserved_funds = 0;
        }
    } else if type_url.ends_with("FundsDeposited") {
        if let Ok(event) = FundsDeposited::decode(event_any.value.as_slice()) {
            if let Some(balance) = event.new_balance {
                state.bankroll = balance.amount;
            }
        }
    } else if type_url.ends_with("FundsWithdrawn") {
        if let Ok(event) = FundsWithdrawn::decode(event_any.value.as_slice()) {
            if let Some(balance) = event.new_balance {
                state.bankroll = balance.amount;
            }
        }
    } else if type_url.ends_with("FundsReserved") {
        if let Ok(event) = FundsReserved::decode(event_any.value.as_slice()) {
            if let Some(balance) = event.new_reserved_balance {
                state.reserved_funds = balance.amount;
            }
            if let (Some(amount), table_root) = (event.amount, event.table_root) {
                let table_key = hex::encode(&table_root);
                state.table_reservations.insert(table_key, amount.amount);
            }
        }
    } else if type_url.ends_with("FundsReleased") {
        if let Ok(event) = FundsReleased::decode(event_any.value.as_slice()) {
            if let Some(balance) = event.new_reserved_balance {
                state.reserved_funds = balance.amount;
            }
            let table_key = hex::encode(&event.table_root);
            state.table_reservations.remove(&table_key);
        }
    } else if type_url.ends_with("FundsTransferred") {
        if let Ok(event) = FundsTransferred::decode(event_any.value.as_slice()) {
            if let Some(balance) = event.new_balance {
                state.bankroll = balance.amount;
            }
        }
    }
}
