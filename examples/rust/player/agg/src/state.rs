//! Player aggregate state.
//!
//! DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//!      Update documentation when making changes to StateRouter patterns.

use std::collections::HashMap;
use std::sync::LazyLock;

use angzarr_client::proto::examples::{
    FundsDeposited, FundsReleased, FundsReserved, FundsTransferred, FundsWithdrawn,
    PlayerRegistered, PlayerState as ProtoPlayerState, PlayerType,
};
use angzarr_client::proto::EventBook;
use angzarr_client::StateRouter;
use angzarr_client::UnpackAny;

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

// Event applier functions for StateRouter

// docs:start:state_router
fn apply_registered(state: &mut PlayerState, event: PlayerRegistered) {
    state.player_id = format!("player_{}", event.email);
    state.display_name = event.display_name;
    state.email = event.email;
    state.player_type = PlayerType::try_from(event.player_type).unwrap_or_default();
    state.ai_model_id = event.ai_model_id;
    state.status = "active".to_string();
    state.bankroll = 0;
    state.reserved_funds = 0;
}

fn apply_deposited(state: &mut PlayerState, event: FundsDeposited) {
    if let Some(balance) = event.new_balance {
        state.bankroll = balance.amount;
    }
}

fn apply_withdrawn(state: &mut PlayerState, event: FundsWithdrawn) {
    if let Some(balance) = event.new_balance {
        state.bankroll = balance.amount;
    }
}

fn apply_reserved(state: &mut PlayerState, event: FundsReserved) {
    if let Some(balance) = event.new_reserved_balance {
        state.reserved_funds = balance.amount;
    }
    if let (Some(amount), table_root) = (event.amount, event.table_root) {
        let table_key = hex::encode(&table_root);
        state.table_reservations.insert(table_key, amount.amount);
    }
}

fn apply_released(state: &mut PlayerState, event: FundsReleased) {
    if let Some(balance) = event.new_reserved_balance {
        state.reserved_funds = balance.amount;
    }
    let table_key = hex::encode(&event.table_root);
    state.table_reservations.remove(&table_key);
}

fn apply_transferred(state: &mut PlayerState, event: FundsTransferred) {
    if let Some(balance) = event.new_balance {
        state.bankroll = balance.amount;
    }
}

/// StateRouter for fluent state reconstruction.
static STATE_ROUTER: LazyLock<StateRouter<PlayerState>> = LazyLock::new(|| {
    StateRouter::new()
        .on::<PlayerRegistered>("PlayerRegistered", apply_registered)
        .on::<FundsDeposited>("FundsDeposited", apply_deposited)
        .on::<FundsWithdrawn>("FundsWithdrawn", apply_withdrawn)
        .on::<FundsReserved>("FundsReserved", apply_reserved)
        .on::<FundsReleased>("FundsReleased", apply_released)
        .on::<FundsTransferred>("FundsTransferred", apply_transferred)
});
// docs:end:state_router

/// Rebuild player state from event history.
pub fn rebuild_state(event_book: &EventBook) -> PlayerState {
    // Start from snapshot if available
    if let Some(snapshot) = &event_book.snapshot {
        if let Some(snapshot_any) = &snapshot.state {
            if let Ok(proto_state) = snapshot_any.unpack::<ProtoPlayerState>() {
                let mut state = apply_snapshot(&proto_state);
                // Apply events since snapshot
                for page in &event_book.pages {
                    if let Some(event) = &page.event {
                        STATE_ROUTER.apply_single(&mut state, event);
                    }
                }
                return state;
            }
        }
    }

    STATE_ROUTER.with_event_book(event_book)
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
