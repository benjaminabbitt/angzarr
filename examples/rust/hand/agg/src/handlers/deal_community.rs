//! DealCommunityCards command handler.

use angzarr_client::proto::examples::{BettingPhase, CommunityCardsDealt, DealCommunityCards};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::game_rules;
use crate::state::HandState;

pub fn handle_deal_community_cards(
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

    // Check if variant uses community cards
    let rules = game_rules::get_rules(state.game_variant);
    if !rules.uses_community_cards() {
        return Err(CommandRejectedError::new(
            "Community cards not used in this variant",
        ));
    }

    let cmd: DealCommunityCards = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    // Determine next phase and cards to deal
    let (new_phase, cards_to_deal) = match state.current_phase {
        BettingPhase::Preflop => (BettingPhase::Flop, 3),
        BettingPhase::Flop => (BettingPhase::Turn, 1),
        BettingPhase::Turn => (BettingPhase::River, 1),
        _ => {
            return Err(CommandRejectedError::new(
                "Cannot deal more community cards",
            ))
        }
    };

    // Validate count if specified
    if cmd.count > 0 && cmd.count as usize != cards_to_deal {
        return Err(CommandRejectedError::new("Invalid card count for phase"));
    }

    // Check we have enough cards in deck
    if state.remaining_deck.len() < cards_to_deal {
        return Err(CommandRejectedError::new("Not enough cards in deck"));
    }

    // Deal cards from remaining deck
    let new_cards: Vec<_> = state.remaining_deck[..cards_to_deal].to_vec();
    let mut all_community = state.community_cards.clone();
    all_community.extend(new_cards.clone());

    let event = CommunityCardsDealt {
        cards: new_cards,
        phase: new_phase as i32,
        all_community_cards: all_community,
        dealt_at: Some(angzarr_client::now()),
    };

    let event_any = pack_event(&event, "examples.CommunityCardsDealt");

    Ok(new_event_book(command_book, seq, event_any))
}
