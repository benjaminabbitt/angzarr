//! Hand aggregate state.

use std::collections::HashMap;
use std::sync::LazyLock;

use angzarr_client::proto::event_page::Payload;
use angzarr_client::proto::examples::{
    ActionTaken, ActionType, BettingPhase, BettingRoundComplete, BlindPosted, Card, CardsDealt,
    CommunityCardsDealt, DrawCompleted, GameVariant, HandComplete, HandState as ProtoHandState,
    PotAwarded, ShowdownStarted,
};
use angzarr_client::proto::EventBook;
use angzarr_client::StateRouter;
use angzarr_client::UnpackAny;

/// Player's state in the hand.
#[derive(Debug, Clone, Default)]
pub struct PlayerHandState {
    pub player_root: Vec<u8>,
    pub position: i32,
    pub hole_cards: Vec<Card>,
    pub stack: i64,
    pub bet_this_round: i64,
    pub total_invested: i64,
    pub has_acted: bool,
    pub has_folded: bool,
    pub is_all_in: bool,
}

/// Pot state.
#[derive(Debug, Clone, Default)]
pub struct PotState {
    pub amount: i64,
    pub eligible_players: Vec<Vec<u8>>,
    pub pot_type: String,
}

/// Hand aggregate state rebuilt from events.
#[derive(Debug, Default, Clone)]
pub struct HandState {
    pub hand_id: String,
    pub table_root: Vec<u8>,
    pub hand_number: i64,
    pub game_variant: GameVariant,

    // Deck state
    pub remaining_deck: Vec<Card>,

    // Player state
    pub players: HashMap<String, PlayerHandState>, // player_root_hex -> state

    // Community cards
    pub community_cards: Vec<Card>,

    // Betting state
    pub current_phase: BettingPhase,
    pub action_on_position: i32,
    pub current_bet: i64,
    pub min_raise: i64,
    pub pots: Vec<PotState>,

    // Positions
    pub dealer_position: i32,
    pub small_blind_position: i32,
    pub big_blind_position: i32,

    pub status: String, // "dealing", "betting", "showdown", "complete"
}

impl HandState {
    /// Check if the hand exists.
    pub fn exists(&self) -> bool {
        !self.hand_id.is_empty()
    }

    /// Check if the hand is complete.
    pub fn is_complete(&self) -> bool {
        self.status == "complete"
    }

    /// Count active (non-folded) players.
    pub fn active_player_count(&self) -> usize {
        self.players.values().filter(|p| !p.has_folded).count()
    }

    /// Get player by root.
    pub fn get_player(&self, player_root: &[u8]) -> Option<&PlayerHandState> {
        let key = hex::encode(player_root);
        self.players.get(&key)
    }

    /// Get mutable player by root.
    pub fn get_player_mut(&mut self, player_root: &[u8]) -> Option<&mut PlayerHandState> {
        let key = hex::encode(player_root);
        self.players.get_mut(&key)
    }

    /// Get total pot amount.
    pub fn total_pot(&self) -> i64 {
        self.pots.iter().map(|p| p.amount).sum()
    }
}

// Event applier functions for StateRouter

fn apply_cards_dealt(state: &mut HandState, event: CardsDealt) {
    state.hand_id = format!("{}_{}", hex::encode(&state.table_root), event.hand_number);
    state.table_root = event.table_root;
    state.hand_number = event.hand_number;
    state.game_variant = GameVariant::try_from(event.game_variant).unwrap_or_default();
    state.dealer_position = event.dealer_position;
    state.remaining_deck = event.remaining_deck;
    state.current_phase = BettingPhase::Preflop;
    state.status = "betting".to_string();

    // Initialize players from PlayerInHand messages
    for p in &event.players {
        let key = hex::encode(&p.player_root);
        state.players.insert(
            key,
            PlayerHandState {
                player_root: p.player_root.clone(),
                position: p.position,
                stack: p.stack,
                ..Default::default()
            },
        );
    }

    // Apply hole cards
    for pc in &event.player_cards {
        let key = hex::encode(&pc.player_root);
        if let Some(player) = state.players.get_mut(&key) {
            player.hole_cards = pc.cards.clone();
        }
    }
}

fn apply_blind_posted(state: &mut HandState, event: BlindPosted) {
    let key = hex::encode(&event.player_root);
    if let Some(player) = state.players.get_mut(&key) {
        player.stack = event.player_stack;
        player.bet_this_round += event.amount;
        player.total_invested += event.amount;
    }
    if let Some(pot) = state.pots.first_mut() {
        pot.amount = event.pot_total;
    }
    if event.amount > state.current_bet {
        state.current_bet = event.amount;
    }
    // Track min_raise as the big blind (highest blind posted)
    if event.amount > state.min_raise {
        state.min_raise = event.amount;
    }
}

fn apply_action_taken(state: &mut HandState, event: ActionTaken) {
    let key = hex::encode(&event.player_root);
    if let Some(player) = state.players.get_mut(&key) {
        player.stack = event.player_stack;
        player.has_acted = true;

        match ActionType::try_from(event.action).unwrap_or_default() {
            ActionType::Fold => {
                player.has_folded = true;
            }
            ActionType::AllIn => {
                player.is_all_in = true;
                player.bet_this_round += event.amount;
                player.total_invested += event.amount;
            }
            ActionType::Bet | ActionType::Raise | ActionType::Call => {
                player.bet_this_round += event.amount;
                player.total_invested += event.amount;
            }
            _ => {}
        }
    }
    if let Some(pot) = state.pots.first_mut() {
        pot.amount = event.pot_total;
    }
    state.current_bet = event.amount_to_call;
}

fn apply_betting_round_complete(state: &mut HandState, event: BettingRoundComplete) {
    // Reset for next round
    for player in state.players.values_mut() {
        player.bet_this_round = 0;
        player.has_acted = false;
    }
    state.current_bet = 0;

    // Update stacks from snapshot
    for snap in &event.stacks {
        let key = hex::encode(&snap.player_root);
        if let Some(player) = state.players.get_mut(&key) {
            player.stack = snap.stack;
            player.is_all_in = snap.is_all_in;
            player.has_folded = snap.has_folded;
        }
    }

    // For Five Card Draw, transition to Draw phase after preflop
    if state.game_variant == GameVariant::FiveCardDraw {
        let completed = BettingPhase::try_from(event.completed_phase).unwrap_or_default();
        if completed == BettingPhase::Preflop {
            state.current_phase = BettingPhase::Draw;
        }
    }
}

fn apply_community_cards_dealt(state: &mut HandState, event: CommunityCardsDealt) {
    // Remove dealt cards from deck
    let cards_dealt = event.cards.len();
    if state.remaining_deck.len() >= cards_dealt {
        state.remaining_deck = state.remaining_deck[cards_dealt..].to_vec();
    }
    state.community_cards = event.all_community_cards;
    state.current_phase = BettingPhase::try_from(event.phase).unwrap_or_default();
    // Reset betting state for new round
    for player in state.players.values_mut() {
        player.bet_this_round = 0;
        player.has_acted = false;
    }
    state.current_bet = 0;
}

fn apply_draw_completed(state: &mut HandState, event: DrawCompleted) {
    // Update player's hole cards
    let key = hex::encode(&event.player_root);
    if let Some(player) = state.players.get_mut(&key) {
        player.hole_cards = event.new_cards;
    }
    // Remove drawn cards from deck
    let cards_drawn = event.cards_drawn as usize;
    if state.remaining_deck.len() >= cards_drawn {
        state.remaining_deck = state.remaining_deck[cards_drawn..].to_vec();
    }
}

fn apply_showdown_started(state: &mut HandState, _event: ShowdownStarted) {
    state.status = "showdown".to_string();
}

fn apply_pot_awarded(state: &mut HandState, event: PotAwarded) {
    for winner in &event.winners {
        let key = hex::encode(&winner.player_root);
        if let Some(player) = state.players.get_mut(&key) {
            player.stack += winner.amount;
        }
    }
}

fn apply_hand_complete(state: &mut HandState, event: HandComplete) {
    state.status = "complete".to_string();
    // Update final stacks
    for snap in &event.final_stacks {
        let key = hex::encode(&snap.player_root);
        if let Some(player) = state.players.get_mut(&key) {
            player.stack = snap.stack;
        }
    }
}

/// Default state factory for StateRouter.
fn new_hand_state() -> HandState {
    HandState {
        pots: vec![PotState {
            pot_type: "main".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// StateRouter for fluent state reconstruction.
///
/// Type names are extracted via reflection using `prost::Name::full_name()`.
static STATE_ROUTER: LazyLock<StateRouter<HandState>> = LazyLock::new(|| {
    StateRouter::with_factory(new_hand_state)
        .on::<CardsDealt>(apply_cards_dealt)
        .on::<BlindPosted>(apply_blind_posted)
        .on::<ActionTaken>(apply_action_taken)
        .on::<BettingRoundComplete>(apply_betting_round_complete)
        .on::<CommunityCardsDealt>(apply_community_cards_dealt)
        .on::<DrawCompleted>(apply_draw_completed)
        .on::<ShowdownStarted>(apply_showdown_started)
        .on::<PotAwarded>(apply_pot_awarded)
        .on::<HandComplete>(apply_hand_complete)
});

/// Rebuild hand state from event history.
pub fn rebuild_state(event_book: &EventBook) -> HandState {
    // Start from snapshot if available
    if let Some(snapshot) = &event_book.snapshot {
        if let Some(snapshot_any) = &snapshot.state {
            if let Ok(proto_state) = snapshot_any.unpack::<ProtoHandState>() {
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

fn apply_snapshot(snapshot: &ProtoHandState) -> HandState {
    let mut players = HashMap::new();
    for p in &snapshot.players {
        let key = hex::encode(&p.player_root);
        players.insert(
            key,
            PlayerHandState {
                player_root: p.player_root.clone(),
                position: p.position,
                hole_cards: p.hole_cards.clone(),
                stack: p.stack,
                bet_this_round: p.bet_this_round,
                total_invested: p.total_invested,
                has_acted: p.has_acted,
                has_folded: p.has_folded,
                is_all_in: p.is_all_in,
            },
        );
    }

    let pots = snapshot
        .pots
        .iter()
        .map(|pot| PotState {
            amount: pot.amount,
            eligible_players: pot.eligible_players.clone(),
            pot_type: pot.pot_type.clone(),
        })
        .collect();

    HandState {
        hand_id: snapshot.hand_id.clone(),
        table_root: snapshot.table_root.clone(),
        hand_number: snapshot.hand_number,
        game_variant: GameVariant::try_from(snapshot.game_variant).unwrap_or_default(),
        remaining_deck: snapshot.remaining_deck.clone(),
        players,
        community_cards: snapshot.community_cards.clone(),
        current_phase: BettingPhase::try_from(snapshot.current_phase).unwrap_or_default(),
        action_on_position: snapshot.action_on_position,
        current_bet: snapshot.current_bet,
        min_raise: snapshot.min_raise,
        pots,
        dealer_position: snapshot.dealer_position,
        small_blind_position: snapshot.small_blind_position,
        big_blind_position: snapshot.big_blind_position,
        status: snapshot.status.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr_client::pack_event;
    use angzarr_client::proto::event_page;

    #[test]
    fn test_community_cards_dealt_applies_correctly() {
        // Create a CommunityCardsDealt event
        let event = CommunityCardsDealt {
            cards: vec![
                Card { suit: 0, rank: 10 },
                Card { suit: 1, rank: 11 },
                Card { suit: 2, rank: 12 },
            ],
            phase: BettingPhase::Flop as i32,
            all_community_cards: vec![
                Card { suit: 0, rank: 10 },
                Card { suit: 1, rank: 11 },
                Card { suit: 2, rank: 12 },
            ],
            dealt_at: None,
        };

        let event_any = pack_event(&event, "examples.CommunityCardsDealt");

        // Verify STATE_ROUTER applies the event correctly
        let mut state = HandState::default();
        STATE_ROUTER.apply_single(&mut state, &event_any);
        assert_eq!(state.community_cards.len(), 3, "STATE_ROUTER apply failed");
        assert_eq!(state.current_phase, BettingPhase::Flop, "phase not updated");
    }

    #[test]
    fn test_rebuild_from_event_book() {
        use angzarr_client::proto::{EventBook, EventPage, Cover, Uuid};

        // Create CardsDealt event first
        let cards_dealt = CardsDealt {
            table_root: vec![1, 2, 3],
            hand_number: 1,
            game_variant: GameVariant::TexasHoldem as i32,
            dealer_position: 0,
            players: vec![],
            player_cards: vec![],
            remaining_deck: (0..52).map(|i| Card { suit: i / 13, rank: i % 13 }).collect(),
            dealt_at: None,
        };

        // Create CommunityCardsDealt event
        let community = CommunityCardsDealt {
            cards: vec![Card { suit: 0, rank: 10 }],
            phase: BettingPhase::Flop as i32,
            all_community_cards: vec![Card { suit: 0, rank: 10 }],
            dealt_at: None,
        };

        let event_book = EventBook {
            cover: Some(Cover {
                domain: "hand".to_string(),
                root: Some(Uuid { value: vec![1, 2, 3] }),
                ..Default::default()
            }),
            pages: vec![
                EventPage {
                    sequence: 0,
                    payload: Some(event_page::Payload::Event(
                        pack_event(&cards_dealt, "examples.CardsDealt")
                    )),
                    created_at: None,
                },
                EventPage {
                    sequence: 1,
                    payload: Some(event_page::Payload::Event(
                        pack_event(&community, "examples.CommunityCardsDealt")
                    )),
                    created_at: None,
                },
            ],
            snapshot: None,
            next_sequence: 2,
        };

        let state = rebuild_state(&event_book);

        assert_eq!(state.current_phase, BettingPhase::Flop, "phase should be Flop after community dealt");
        assert_eq!(state.community_cards.len(), 1, "should have 1 community card");
    }
}
