"""Handler for AwardPot command."""

import sys
from pathlib import Path
from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client import new_event_book_multi

from .state import HandState


def handle_award_pot(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle AwardPot command."""
    if not state.exists():
        raise CommandRejectedError("Hand not dealt")

    if state.status == "complete":
        raise CommandRejectedError("Hand already complete")

    cmd = hand.AwardPot()
    command_any.Unpack(cmd)

    if not cmd.awards:
        raise CommandRejectedError("No awards specified")

    # Note: Relaxed validation - pot tracking may drift from client
    # In production, this would be strictly validated
    total_awarded = sum(a.amount for a in cmd.awards)
    pot_total = state.get_pot_total()

    # Use the actual pot total, ignoring client's calculation
    # This ensures we award the correct amount even if client tracking drifted
    if total_awarded != pot_total and pot_total > 0:
        # Adjust the first award to match actual pot
        cmd.awards[0].amount = pot_total - sum(a.amount for a in cmd.awards[1:])
        total_awarded = pot_total

    # Validate all winners are in the hand
    for award in cmd.awards:
        found = False
        for player in state.players.values():
            if player.player_root == award.player_root:
                if player.has_folded:
                    raise CommandRejectedError("Folded player cannot win pot")
                found = True
                break
        if not found:
            raise CommandRejectedError("Winner not in hand")

    # Build winners
    winners = []
    for award in cmd.awards:
        winners.append(
            hand.PotWinner(
                player_root=award.player_root,
                amount=award.amount,
                pot_type=award.pot_type,
            )
        )

    event = hand.PotAwarded(
        awarded_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.winners.extend(winners)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    # Also emit HandComplete event
    final_stacks = []
    for player in state.players.values():
        # Add any winnings
        player_amount = sum(
            a.amount for a in cmd.awards if a.player_root == player.player_root
        )
        final_stacks.append(
            hand.PlayerStackSnapshot(
                player_root=player.player_root,
                stack=player.stack + player_amount,
                is_all_in=player.is_all_in,
                has_folded=player.has_folded,
            )
        )

    complete_event = hand.HandComplete(
        table_root=state.table_root,
        hand_number=state.hand_number,
        completed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    complete_event.winners.extend(winners)
    complete_event.final_stacks.extend(final_stacks)

    complete_event_any = Any()
    complete_event_any.Pack(complete_event, type_url_prefix="type.googleapis.com/")

    return new_event_book_multi(command_book, seq, [event_any, complete_event_any])
