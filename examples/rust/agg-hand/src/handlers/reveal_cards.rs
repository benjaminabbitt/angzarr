//! RevealCards command handler.

use angzarr_client::proto::examples::{CardsMucked, CardsRevealed, HandRanking, RevealCards};
use angzarr_client::proto::{event_page, CommandBook, EventBook, EventPage};
use angzarr_client::{pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::game_rules::get_rules;
use crate::state::HandState;

pub fn handle_reveal_cards(
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

    let cmd: RevealCards = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    let player = state
        .get_player(&cmd.player_root)
        .ok_or_else(|| CommandRejectedError::new("Player not in hand"))?;

    if player.has_folded {
        return Err(CommandRejectedError::new("Player has folded"));
    }

    let event_any = if cmd.muck {
        let event = CardsMucked {
            player_root: cmd.player_root,
            mucked_at: Some(angzarr_client::now()),
        };
        pack_event(&event, "examples.CardsMucked")
    } else {
        // Use game rules to evaluate hand properly
        let rules = get_rules(state.game_variant);
        let hand_rank = rules.evaluate_hand(&player.hole_cards, &state.community_cards);

        let ranking = HandRanking {
            rank_type: hand_rank.rank_type as i32,
            kickers: hand_rank.kickers.into_iter().map(|r| r as i32).collect(),
            score: hand_rank.score,
        };

        let event = CardsRevealed {
            player_root: cmd.player_root,
            cards: player.hole_cards.clone(),
            ranking: Some(ranking),
            revealed_at: Some(angzarr_client::now()),
        };
        pack_event(&event, "examples.CardsRevealed")
    };

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
