//! AwardPot command handler.

use angzarr_client::proto::examples::{
    AwardPot, HandComplete, PlayerStackSnapshot, PotAwarded, PotWinner,
};
use angzarr_client::proto::{CommandBook, EventBook};
use angzarr_client::{new_event_book_multi, pack_event, CommandRejectedError, CommandResult, UnpackAny};
use prost_types::Any;

use crate::state::HandState;

fn guard(state: &HandState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Hand does not exist"));
    }
    if state.is_complete() {
        return Err(CommandRejectedError::new("Hand already complete"));
    }
    Ok(())
}

fn validate(cmd: &AwardPot, state: &HandState) -> CommandResult<()> {
    if cmd.awards.is_empty() {
        return Err(CommandRejectedError::new("No awards specified"));
    }

    let mut total_awarded = 0i64;
    for award in &cmd.awards {
        let player = state
            .get_player(&award.player_root)
            .ok_or_else(|| CommandRejectedError::new("Award to player not in hand"))?;

        if player.has_folded {
            return Err(CommandRejectedError::new("Cannot award to folded player"));
        }
        total_awarded += award.amount;
    }

    if total_awarded > state.total_pot() {
        return Err(CommandRejectedError::new("Awards exceed pot total"));
    }

    Ok(())
}

fn compute(cmd: &AwardPot, state: &HandState) -> (PotAwarded, HandComplete) {
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

    let final_stacks: Vec<PlayerStackSnapshot> = state
        .players
        .values()
        .map(|player| {
            let mut final_stack = player.stack;
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

    (pot_awarded, hand_complete)
}

pub fn handle_award_pot(
    command_book: &CommandBook,
    command_any: &Any,
    state: &HandState,
    seq: u32,
) -> CommandResult<EventBook> {
    let cmd: AwardPot = command_any
        .unpack()
        .map_err(|e| CommandRejectedError::new(format!("Failed to decode command: {}", e)))?;

    guard(state)?;
    validate(&cmd, state)?;

    let (pot_awarded, hand_complete) = compute(&cmd, state);

    Ok(new_event_book_multi(
        command_book,
        seq,
        vec![
            pack_event(&pot_awarded, "examples.PotAwarded"),
            pack_event(&hand_complete, "examples.HandComplete"),
        ],
    ))
}
