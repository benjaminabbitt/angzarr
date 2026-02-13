"""Handler for EndHand command."""

import sys
from pathlib import Path
from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client import new_event_book

from .state import TableState


def handle_end_hand(
    command_book: types.CommandBook,
    command_any: Any,
    state: TableState,
    seq: int,
) -> types.EventBook:
    """Handle EndHand command."""
    if not state.exists():
        raise CommandRejectedError("Table does not exist")

    if state.status != "in_hand":
        raise CommandRejectedError("No hand in progress")

    cmd = table.EndHand()
    command_any.Unpack(cmd)

    if cmd.hand_root != state.current_hand_root:
        raise CommandRejectedError("Hand root mismatch")

    # Calculate stack changes from results
    stack_changes = {}
    for result in cmd.results:
        player_hex = result.winner_root.hex()
        if player_hex not in stack_changes:
            stack_changes[player_hex] = 0
        stack_changes[player_hex] += result.amount

    event = table.HandEnded(
        hand_root=cmd.hand_root,
        stack_changes=stack_changes,
        ended_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.results.extend(cmd.results)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    return new_event_book(command_book, seq, event_any)
