"""Handler for RequestAction command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

import sys
from pathlib import Path

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player

from .state import PlayerState


def handle_request_action(
    command_book: types.CommandBook,
    command_any: Any,
    state: PlayerState,
    seq: int,
) -> types.EventBook:
    """Handle RequestAction command - emit ActionRequested event.

    For human players, this triggers the client to prompt for input.
    For AI players, the process manager will handle calling the AI sidecar.
    """
    if not state.exists():
        raise CommandRejectedError("Player does not exist")

    cmd = player.RequestAction()
    command_any.Unpack(cmd)

    # Calculate deadline
    now_ts = int(datetime.now(timezone.utc).timestamp())
    deadline_ts = now_ts + cmd.timeout_seconds

    # Get player root from cover
    player_root = b""
    if command_book.cover and command_book.cover.root:
        player_root = command_book.cover.root.value

    event = player.ActionRequested(
        hand_root=cmd.hand_root,
        table_root=cmd.table_root,
        player_root=player_root,
        player_type=state.player_type,
        amount_to_call=cmd.amount_to_call,
        min_raise=cmd.min_raise,
        max_raise=cmd.max_raise,
        pot_size=cmd.pot_size,
        phase=cmd.phase,
        deadline=Timestamp(seconds=deadline_ts),
    )
    event.hole_cards.extend(cmd.hole_cards)
    event.community_cards.extend(cmd.community_cards)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=now_ts),
            )
        ],
    )
