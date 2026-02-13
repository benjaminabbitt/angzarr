"""Handler for JoinTable command."""

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


def handle_join_table(
    command_book: types.CommandBook,
    command_any: Any,
    state: TableState,
    seq: int,
) -> types.EventBook:
    """Handle JoinTable command."""
    if not state.exists():
        raise CommandRejectedError("Table does not exist")

    cmd = table.JoinTable()
    command_any.Unpack(cmd)

    if not cmd.player_root:
        raise CommandRejectedError("player_root is required")

    # Check if player already seated
    if state.find_player_seat(cmd.player_root):
        raise CommandRejectedError("Player already seated at table")

    # Check if table is full
    if state.is_full():
        raise CommandRejectedError("Table is full")

    # Validate buy-in
    if cmd.buy_in_amount < state.min_buy_in:
        raise CommandRejectedError(f"Buy-in must be at least {state.min_buy_in}")
    if cmd.buy_in_amount > state.max_buy_in:
        raise CommandRejectedError(f"Buy-in cannot exceed {state.max_buy_in}")

    # Check if preferred seat is explicitly requested and occupied
    if cmd.preferred_seat >= 0 and state.get_seat(cmd.preferred_seat) is not None:
        raise CommandRejectedError("Seat is occupied")

    # Find seat
    seat_position = state.find_available_seat(cmd.preferred_seat)
    if seat_position is None:
        raise CommandRejectedError("No available seat")

    event = table.PlayerJoined(
        player_root=cmd.player_root,
        seat_position=seat_position,
        buy_in_amount=cmd.buy_in_amount,
        stack=cmd.buy_in_amount,
        joined_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
