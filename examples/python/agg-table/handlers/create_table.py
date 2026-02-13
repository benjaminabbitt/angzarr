"""Handler for CreateTable command."""

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


def handle_create_table(
    command_book: types.CommandBook,
    command_any: Any,
    state: TableState,
    seq: int,
) -> types.EventBook:
    """Handle CreateTable command."""
    if state.exists():
        raise CommandRejectedError("Table already exists")

    cmd = table.CreateTable()
    command_any.Unpack(cmd)

    if not cmd.table_name:
        raise CommandRejectedError("table_name is required")
    if cmd.small_blind <= 0:
        raise CommandRejectedError("small_blind must be positive")
    if cmd.big_blind <= 0:
        raise CommandRejectedError("big_blind must be positive")
    if cmd.big_blind < cmd.small_blind:
        raise CommandRejectedError("big_blind must be >= small_blind")
    if cmd.max_players < 2 or cmd.max_players > 10:
        raise CommandRejectedError("max_players must be between 2 and 10")

    event = table.TableCreated(
        table_name=cmd.table_name,
        game_variant=cmd.game_variant,
        small_blind=cmd.small_blind,
        big_blind=cmd.big_blind,
        min_buy_in=cmd.min_buy_in or cmd.big_blind * 20,
        max_buy_in=cmd.max_buy_in or cmd.big_blind * 100,
        max_players=cmd.max_players or 9,
        action_timeout_seconds=cmd.action_timeout_seconds or 30,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
