"""Handler for WithdrawFunds command."""

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


def handle_withdraw_funds(
    command_book: types.CommandBook,
    command_any: Any,
    state: PlayerState,
    seq: int,
) -> types.EventBook:
    """Handle WithdrawFunds command."""
    if not state.exists():
        raise CommandRejectedError("Player does not exist")

    cmd = player.WithdrawFunds()
    command_any.Unpack(cmd)

    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")

    available = state.available_balance()
    if amount > available:
        raise CommandRejectedError("Insufficient funds")

    new_balance = state.bankroll - amount

    event = player.FundsWithdrawn(
        amount=cmd.amount,
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        withdrawn_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
