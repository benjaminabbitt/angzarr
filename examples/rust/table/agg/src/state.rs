//! Table aggregate state.

use std::collections::HashMap;
use std::sync::LazyLock;

use angzarr_client::proto::event_page::Payload;
use angzarr_client::proto::examples::{
    ChipsAdded, GameVariant, HandEnded, HandStarted, PlayerJoined, PlayerLeft, PlayerSatIn,
    PlayerSatOut, TableCreated, TableState as ProtoTableState,
};
use angzarr_client::proto::EventBook;
use angzarr_client::StateRouter;
use angzarr_client::UnpackAny;

/// Seat state at the table.
#[derive(Debug, Clone)]
pub struct SeatState {
    pub position: i32,
    pub player_root: Vec<u8>,
    pub stack: i64,
    pub is_active: bool,
    pub is_sitting_out: bool,
}

/// Table aggregate state rebuilt from events.
#[derive(Debug, Default, Clone)]
pub struct TableState {
    pub table_id: String,
    pub table_name: String,
    pub game_variant: GameVariant,
    pub small_blind: i64,
    pub big_blind: i64,
    pub min_buy_in: i64,
    pub max_buy_in: i64,
    pub max_players: i32,
    pub action_timeout_seconds: i32,
    pub seats: HashMap<i32, SeatState>, // position -> seat
    pub dealer_position: i32,
    pub hand_count: i64,
    pub current_hand_root: Vec<u8>,
    pub status: String, // "waiting", "in_hand", "paused"
}

impl TableState {
    /// Check if the table exists.
    pub fn exists(&self) -> bool {
        !self.table_id.is_empty()
    }

    /// Get player count.
    pub fn player_count(&self) -> usize {
        self.seats.len()
    }

    /// Get active (not sitting out) player count.
    pub fn active_player_count(&self) -> usize {
        self.seats.values().filter(|s| !s.is_sitting_out).count()
    }

    /// Find seat by player root.
    pub fn find_seat_by_player(&self, player_root: &[u8]) -> Option<i32> {
        let player_hex = hex::encode(player_root);
        self.seats.iter().find_map(|(pos, seat)| {
            if hex::encode(&seat.player_root) == player_hex {
                Some(*pos)
            } else {
                None
            }
        })
    }

    /// Get next available seat.
    pub fn next_available_seat(&self) -> Option<i32> {
        for i in 0..self.max_players {
            if !self.seats.contains_key(&i) {
                return Some(i);
            }
        }
        None
    }
}

// Event applier functions for StateRouter

fn apply_table_created(state: &mut TableState, event: TableCreated) {
    state.table_id = format!("table_{}", event.table_name);
    state.table_name = event.table_name;
    state.game_variant = GameVariant::try_from(event.game_variant).unwrap_or_default();
    state.small_blind = event.small_blind;
    state.big_blind = event.big_blind;
    state.min_buy_in = event.min_buy_in;
    state.max_buy_in = event.max_buy_in;
    state.max_players = event.max_players;
    state.action_timeout_seconds = event.action_timeout_seconds;
    state.dealer_position = 0;
    state.hand_count = 0;
    state.status = "waiting".to_string();
}

fn apply_player_joined(state: &mut TableState, event: PlayerJoined) {
    state.seats.insert(
        event.seat_position,
        SeatState {
            position: event.seat_position,
            player_root: event.player_root,
            stack: event.stack,
            is_active: true,
            is_sitting_out: false,
        },
    );
}

fn apply_player_left(state: &mut TableState, event: PlayerLeft) {
    state.seats.remove(&event.seat_position);
}

fn apply_player_sat_out(state: &mut TableState, event: PlayerSatOut) {
    if let Some(pos) = state.find_seat_by_player(&event.player_root) {
        if let Some(seat) = state.seats.get_mut(&pos) {
            seat.is_sitting_out = true;
        }
    }
}

fn apply_player_sat_in(state: &mut TableState, event: PlayerSatIn) {
    if let Some(pos) = state.find_seat_by_player(&event.player_root) {
        if let Some(seat) = state.seats.get_mut(&pos) {
            seat.is_sitting_out = false;
        }
    }
}

fn apply_hand_started(state: &mut TableState, event: HandStarted) {
    state.current_hand_root = event.hand_root;
    state.hand_count = event.hand_number;
    state.dealer_position = event.dealer_position;
    state.status = "in_hand".to_string();
}

fn apply_hand_ended(state: &mut TableState, event: HandEnded) {
    state.current_hand_root.clear();
    state.status = "waiting".to_string();
    // Apply stack changes
    for (player_hex, delta) in &event.stack_changes {
        for seat in state.seats.values_mut() {
            if hex::encode(&seat.player_root) == *player_hex {
                seat.stack += delta;
                break;
            }
        }
    }
}

fn apply_chips_added(state: &mut TableState, event: ChipsAdded) {
    if let Some(pos) = state.find_seat_by_player(&event.player_root) {
        if let Some(seat) = state.seats.get_mut(&pos) {
            seat.stack = event.new_stack;
        }
    }
}

/// StateRouter for fluent state reconstruction.
///
/// Type names are extracted via reflection using `prost::Name::full_name()`.
static STATE_ROUTER: LazyLock<StateRouter<TableState>> = LazyLock::new(|| {
    StateRouter::new()
        .on::<TableCreated>(apply_table_created)
        .on::<PlayerJoined>(apply_player_joined)
        .on::<PlayerLeft>(apply_player_left)
        .on::<PlayerSatOut>(apply_player_sat_out)
        .on::<PlayerSatIn>(apply_player_sat_in)
        .on::<HandStarted>(apply_hand_started)
        .on::<HandEnded>(apply_hand_ended)
        .on::<ChipsAdded>(apply_chips_added)
});

/// Rebuild table state from event history.
pub fn rebuild_state(event_book: &EventBook) -> TableState {
    // Start from snapshot if available
    if let Some(snapshot) = &event_book.snapshot {
        if let Some(snapshot_any) = &snapshot.state {
            if let Ok(proto_state) = snapshot_any.unpack::<ProtoTableState>() {
                let mut state = apply_snapshot(&proto_state);
                // Apply events since snapshot
                for page in &event_book.pages {
                    if let Some(Payload::Event(event)) = &page.payload {
                        STATE_ROUTER.apply_single(&mut state, event);
                    }
                }
                return state;
            }
        }
    }

    STATE_ROUTER.with_event_book(event_book)
}

fn apply_snapshot(snapshot: &ProtoTableState) -> TableState {
    let mut seats = HashMap::new();
    for seat in &snapshot.seats {
        let stack = seat.stack.as_ref().map(|c| c.amount).unwrap_or(0);
        seats.insert(
            seat.position,
            SeatState {
                position: seat.position,
                player_root: seat.player_root.clone(),
                stack,
                is_active: seat.is_active,
                is_sitting_out: seat.is_sitting_out,
            },
        );
    }

    TableState {
        table_id: snapshot.table_id.clone(),
        table_name: snapshot.table_name.clone(),
        game_variant: GameVariant::try_from(snapshot.game_variant).unwrap_or_default(),
        small_blind: snapshot.small_blind,
        big_blind: snapshot.big_blind,
        min_buy_in: snapshot.min_buy_in,
        max_buy_in: snapshot.max_buy_in,
        max_players: snapshot.max_players,
        action_timeout_seconds: snapshot.action_timeout_seconds,
        seats,
        dealer_position: snapshot.dealer_position,
        hand_count: snapshot.hand_count,
        current_hand_root: snapshot.current_hand_root.clone(),
        status: snapshot.status.clone(),
    }
}
