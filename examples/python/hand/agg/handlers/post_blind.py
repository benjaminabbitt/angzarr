"""Handler for PostBlind command."""

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
from angzarr_client import new_event_book

from .state import HandState


def handle_post_blind(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle PostBlind command."""
    if not state.exists():
        raise CommandRejectedError("Hand not dealt")

    if state.status == "complete":
        raise CommandRejectedError("Hand is complete")

    cmd = hand.PostBlind()
    command_any.Unpack(cmd)

    if not cmd.player_root:
        raise CommandRejectedError("player_root is required")

    # Find the player
    player = None
    for p in state.players.values():
        if p.player_root == cmd.player_root:
            player = p
            break

    if not player:
        raise CommandRejectedError("Player not in hand")

    if player.has_folded:
        raise CommandRejectedError("Player has folded")

    if cmd.amount <= 0:
        raise CommandRejectedError("Blind amount must be positive")

    # Calculate actual amount (might be all-in)
    actual_amount = min(cmd.amount, player.stack)
    new_stack = player.stack - actual_amount
    new_pot_total = state.get_pot_total() + actual_amount

    event = hand.BlindPosted(
        player_root=cmd.player_root,
        blind_type=cmd.blind_type,
        amount=actual_amount,
        player_stack=new_stack,
        pot_total=new_pot_total,
        posted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    return new_event_book(command_book, seq, event_any)
