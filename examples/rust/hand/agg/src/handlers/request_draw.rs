//! RequestDraw command handler (Five Card Draw specific).

use angzarr_client::proto::examples::{BettingPhase, DrawCompleted, GameVariant, RequestDraw};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::{HandState, PlayerHandState};

/// Validated draw parameters.
struct ValidatedDraw {
    indices: Vec<i32>,
}

fn guard(state: &HandState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Hand does not exist"));
    }
    if state.is_complete() {
        return Err(CommandRejectedError::new("Hand already complete"));
    }
    if state.game_variant != GameVariant::FiveCardDraw {
        return Err(CommandRejectedError::new(
            "Draw not supported in this game variant",
        ));
    }
    if state.current_phase != BettingPhase::Draw {
        return Err(CommandRejectedError::new("Not in draw phase"));
    }
    Ok(())
}

fn validate<'a>(
    cmd: &RequestDraw,
    state: &'a HandState,
) -> CommandResult<(&'a PlayerHandState, ValidatedDraw)> {
    let player = state
        .get_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not in hand"))?;

    if player.has_folded {
        return Err(CommandRejectedError::new("Player has folded"));
    }

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

    if indices.len() > state.remaining_deck.len() {
        return Err(CommandRejectedError::new("Not enough cards in deck"));
    }

    Ok((player, ValidatedDraw { indices }))
}

fn compute(
    cmd: &RequestDraw,
    state: &HandState,
    player: &PlayerHandState,
    validated: &ValidatedDraw,
) -> DrawCompleted {
    let cards_to_draw = validated.indices.len();
    let cards_drawn = state.remaining_deck[..cards_to_draw].to_vec();

    let mut new_cards = player.hole_cards.clone();
    for (i, &idx) in validated.indices.iter().enumerate() {
        new_cards[idx as usize] = cards_drawn[i].clone();
    }

    DrawCompleted {
        player_root: cmd.player_root.clone(),
        cards_discarded: cards_to_draw as i32,
        cards_drawn: cards_to_draw as i32,
        new_cards,
        drawn_at: Some(angzarr_client::now()),
    }
}

pub fn handle_request_draw(
    command_book: &CommandBook,
    command_any: &Any,
    state: &HandState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: RequestDraw = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    let (player, validated) = validate(&cmd, state)?;

    let event = compute(&cmd, state, player, &validated);
    let event_any = pack_event(&event, "examples.DrawCompleted");

    Ok(new_event_book(command_book, seq, event_any))
}
