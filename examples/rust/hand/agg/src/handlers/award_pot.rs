//! AwardPot command handler.

use angzarr_client::proto::examples::{
    AwardPot, HandComplete, PlayerStackSnapshot, PotAwarded, PotWinner,
};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book_multi, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::HandState;

pub fn handle_award_pot(
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

    let cmd: AwardPot = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    if cmd.awards.is_empty() {
        return Err(CommandRejectedError::new("No awards specified"));
    }

    // Validate awards
    let mut total_awarded = 0i64;
    for award in &cmd.awards {
        let player = state.get_player(&award.player_root);
        if player.is_none() {
            return Err(CommandRejectedError::new("Award to player not in hand"));
        }
        if let Some(p) = player {
            if p.has_folded {
                return Err(CommandRejectedError::new("Cannot award to folded player"));
            }
        }
        total_awarded += award.amount;
    }

    // Verify total doesn't exceed pot
    if total_awarded > state.total_pot() {
        return Err(CommandRejectedError::new("Awards exceed pot total"));
    }

    // Build pot winners
    let winners: Vec<PotWinner> = cmd
        .awards
        .iter()
        .map(|award| PotWinner {
            player_root: award.player_root.clone(),
            amount: award.amount,
            pot_type: award.pot_type.clone(),
            winning_hand: None,
        })
        .collect();

    let now = angzarr_client::now();

    // Build final stacks
    let final_stacks: Vec<PlayerStackSnapshot> = state
        .players
        .values()
        .map(|player| {
            let mut final_stack = player.stack;
            // Add any winnings
            for award in &cmd.awards {
                if award.player_root == player.player_root {
                    final_stack += award.amount;
                }
            }
            PlayerStackSnapshot {
                player_root: player.player_root.clone(),
                stack: final_stack,
                is_all_in: player.is_all_in,
                has_folded: player.has_folded,
            }
        })
        .collect();

    // Create events
    let pot_awarded = PotAwarded {
        winners: winners.clone(),
        awarded_at: Some(now.clone()),
    };

    let hand_complete = HandComplete {
        table_root: state.table_root.clone(),
        hand_number: state.hand_number,
        winners,
        final_stacks,
        completed_at: Some(now),
    };

    Ok(new_event_book_multi(
        command_book,
        seq,
        vec![
            pack_event(&pot_awarded, "examples.PotAwarded"),
            pack_event(&hand_complete, "examples.HandComplete"),
        ],
    ))
}
