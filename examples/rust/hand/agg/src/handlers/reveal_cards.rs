//! RevealCards command handler.

use angzarr_client::proto::examples::{CardsMucked, CardsRevealed, HandRanking, RevealCards};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::game_rules::get_rules;
use crate::state::{HandState, PlayerHandState};

fn guard(state: &HandState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Hand does not exist"));
    }
    if state.is_complete() {
        return Err(CommandRejectedError::new("Hand already complete"));
    }
    Ok(())
}

fn validate<'a>(cmd: &RevealCards, state: &'a HandState) -> CommandResult<&'a PlayerHandState> {
    let player = state
        .get_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not in hand"))?;

    if player.has_folded {
        return Err(CommandRejectedError::new("Player has folded"));
    }

    Ok(player)
}

fn compute_muck(cmd: &RevealCards) -> CardsMucked {
    CardsMucked {
        player_root: cmd.player_root.clone(),
        mucked_at: Some(angzarr_client::now()),
    }
}

fn compute_reveal(cmd: &RevealCards, state: &HandState, player: &PlayerHandState) -> CardsRevealed {
    let rules = get_rules(state.game_variant);
    let hand_rank = rules.evaluate_hand(&player.hole_cards, &state.community_cards);

    let ranking = HandRanking {
        rank_type: hand_rank.rank_type as i32,
        kickers: hand_rank.kickers.into_iter().map(|r| r as i32).collect(),
        score: hand_rank.score,
    };

    CardsRevealed {
        player_root: cmd.player_root.clone(),
        cards: player.hole_cards.clone(),
        ranking: Some(ranking),
        revealed_at: Some(angzarr_client::now()),
    }
}

pub fn handle_reveal_cards(
    command_book: &CommandBook,
    command_any: &Any,
    state: &HandState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: RevealCards = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    let player = validate(&cmd, state)?;

    let event_any = if cmd.muck {
        let event = compute_muck(&cmd);
        pack_event(&event, "examples.CardsMucked")
    } else {
        let event = compute_reveal(&cmd, state, player);
        pack_event(&event, "examples.CardsRevealed")
    };

    Ok(new_event_book(command_book, seq, event_any))
}
