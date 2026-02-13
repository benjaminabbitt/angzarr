//! DealCards command handler.

use rand::prelude::*;
use sha2::{Digest, Sha256};

use angzarr_client::proto::examples::{
    Card, CardsDealt, DealCards, GameVariant, PlayerHoleCards, Rank, Suit,
};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::game_rules::get_rules;
use crate::state::HandState;

pub fn handle_deal_cards(
    command_book: &CommandBook,
    command_any: &Any,
    state: &HandState,
    seq: u32,
) -> CommandResult<EventBook> {
    if state.exists() {
        return Err(CommandRejectedError::new("Hand already dealt"));
    }

    let cmd: DealCards = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    if cmd.players.len() < 2 {
        return Err(CommandRejectedError::new("Need at least 2 players"));
    }

    // Create and shuffle deck
    let mut deck = create_deck();
    let seed = if cmd.deck_seed.is_empty() {
        let mut rng = rand::thread_rng();
        let mut seed = [0u8; 32];
        rng.fill(&mut seed);
        seed.to_vec()
    } else {
        cmd.deck_seed.clone()
    };
    shuffle_deck(&mut deck, &seed);

    // Get game rules for variant
    let variant = GameVariant::try_from(cmd.game_variant).unwrap_or(GameVariant::TexasHoldem);
    let rules = get_rules(variant);
    let cards_per_player = rules.hole_card_count();

    let mut player_cards = Vec::new();
    let total_cards_to_deal = cmd.players.len() * cards_per_player;

    // Deal hole cards
    for (i, player) in cmd.players.iter().enumerate() {
        let start = i * cards_per_player;
        let end = start + cards_per_player;
        let cards: Vec<Card> = deck[start..end].to_vec();
        player_cards.push(PlayerHoleCards {
            player_root: player.player_root.clone(),
            cards,
        });
    }

    // Remaining deck for community cards / draws
    let remaining_deck: Vec<Card> = deck[total_cards_to_deal..].to_vec();

    let event = CardsDealt {
        table_root: cmd.table_root,
        hand_number: cmd.hand_number,
        game_variant: cmd.game_variant,
        player_cards,
        dealer_position: cmd.dealer_position,
        players: cmd.players,
        dealt_at: Some(angzarr_client::now()),
        remaining_deck,
    };

    let event_any = pack_event(&event, "examples.CardsDealt");

    Ok(EventBook {
        cover: command_book.cover.clone(),
        pages: vec![EventPage {
            sequence: Some(event_page::Sequence::Num(seq)),
            event: Some(event_any),
            created_at: Some(angzarr_client::now()),
        }],
        snapshot: None,
        next_sequence: 0,
    })
}

/// Create a standard 52-card deck.
fn create_deck() -> Vec<Card> {
    let suits = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
    let ranks = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];

    let mut deck = Vec::with_capacity(52);
    for suit in suits {
        for rank in ranks {
            deck.push(Card {
                suit: suit as i32,
                rank: rank as i32,
            });
        }
    }
    deck
}

/// Shuffle the deck using a seed for determinism.
fn shuffle_deck(deck: &mut Vec<Card>, seed: &[u8]) {
    let hash = Sha256::digest(seed);
    let seed_int = u64::from_be_bytes(hash[..8].try_into().unwrap());
    let mut rng = StdRng::seed_from_u64(seed_int);
    deck.shuffle(&mut rng);
}
