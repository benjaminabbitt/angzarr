//! RequestDraw command handler (Five Card Draw specific).

use angzarr_client::proto::examples::{BettingPhase, DrawCompleted, GameVariant, RequestDraw};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::HandState;

pub fn handle_request_draw(
    command_book: &CommandBook,
    command_any: &Any,
    state: &HandState,
    seq: u32,
) -> CommandResult<EventBook> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Hand does not exist"));
    }
    if state.is_complete() {
        return Err(CommandRejectedError::new("Hand already complete"));
    }

    // Only valid for Five Card Draw
    if state.game_variant != GameVariant::FiveCardDraw {
        return Err(CommandRejectedError::new(
            "Draw not supported in this game variant",
        ));
    }

    // Must be in draw phase
    if state.current_phase != BettingPhase::Draw {
        return Err(CommandRejectedError::new("Not in draw phase"));
    }

    let cmd: RequestDraw = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    let player = state
        .get_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not in hand"))?;

    if player.has_folded {
        return Err(CommandRejectedError::new("Player has folded"));
    }

    // Validate card indices (0-4, no duplicates)
    let mut indices: Vec<i32> = cmd.card_indices.clone();
    indices.sort();
    indices.dedup();

    if indices.len() != cmd.card_indices.len() {
        return Err(CommandRejectedError::new("Duplicate card indices"));
    }

    for &idx in &indices {
        if idx < 0 || idx >= 5 {
            return Err(CommandRejectedError::new("Card index out of range (0-4)"));
        }
    }

    let cards_to_draw = indices.len();
    if cards_to_draw > state.remaining_deck.len() {
        return Err(CommandRejectedError::new("Not enough cards in deck"));
    }

    // Draw new cards from deck
    let cards_drawn = state.remaining_deck[..cards_to_draw].to_vec();

    // Build new hand: keep non-discarded cards, add new cards
    let mut new_cards = player.hole_cards.clone();
    for (i, &idx) in indices.iter().enumerate() {
        new_cards[idx as usize] = cards_drawn[i].clone();
    }

    let event = DrawCompleted {
        player_root: cmd.player_root,
        cards_discarded: cards_to_draw as i32,
        cards_drawn: cards_to_draw as i32,
        new_cards,
        drawn_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.DrawCompleted");

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
