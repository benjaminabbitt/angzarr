"""Handler for ReleaseFunds command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

import sys
from pathlib import Path

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import types_pb2 as poker_types

from .state import PlayerState


def handle_release_funds(
    command_book: types.CommandBook,
    command_any: Any,
    state: PlayerState,
    seq: int,
) -> types.EventBook:
    """Handle ReleaseFunds command - release reserved funds when leaving table."""
    if not state.exists():
        raise CommandRejectedError("Player does not exist")

    cmd = player.ReleaseFunds()
    command_any.Unpack(cmd)

    table_key = cmd.table_root.hex()
    reserved_for_table = state.table_reservations.get(table_key, 0)

    if reserved_for_table == 0:
        raise CommandRejectedError("No funds reserved for this table")

    new_reserved = state.reserved_funds - reserved_for_table
    new_available = state.bankroll - new_reserved

    event = player.FundsReleased(
        amount=poker_types.Currency(amount=reserved_for_table, currency_code="CHIPS"),
        table_root=cmd.table_root,
        new_available_balance=poker_types.Currency(
            amount=new_available, currency_code="CHIPS"
        ),
        new_reserved_balance=poker_types.Currency(
            amount=new_reserved, currency_code="CHIPS"
        ),
        released_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
