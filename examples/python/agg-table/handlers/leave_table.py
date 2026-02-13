"""Handler for LeaveTable command."""

import sys
from pathlib import Path
from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import table_pb2 as table

from .state import TableState


def handle_leave_table(
    command_book: types.CommandBook,
    command_any: Any,
    state: TableState,
    seq: int,
) -> types.EventBook:
    """Handle LeaveTable command."""
    if not state.exists():
        raise CommandRejectedError("Table does not exist")

    cmd = table.LeaveTable()
    command_any.Unpack(cmd)

    if not cmd.player_root:
        raise CommandRejectedError("player_root is required")

    # Find player's seat
    seat = state.find_player_seat(cmd.player_root)
    if not seat:
        raise CommandRejectedError("Player is not seated at table")

    # Cannot leave during a hand (unless folded - handled by process manager)
    if state.status == "in_hand":
        raise CommandRejectedError("Cannot leave table during a hand")

    event = table.PlayerLeft(
        player_root=cmd.player_root,
        seat_position=seat.position,
        chips_cashed_out=seat.stack,
        left_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.poker/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(
                    seconds=int(datetime.now(timezone.utc).timestamp())
                ),
            )
        ],
    )
