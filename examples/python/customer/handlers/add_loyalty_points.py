"""Handler for AddLoyaltyPoints command."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from handlers import CommandRejectedError


def handle_add_loyalty_points(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.CustomerState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle AddLoyaltyPoints command."""
    if not state.name:
        raise CommandRejectedError("Customer does not exist")

    cmd = domains.AddLoyaltyPoints()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")

    new_balance = state.loyalty_points + cmd.points

    log.info(
        "adding_loyalty_points",
        points=cmd.points,
        new_balance=new_balance,
        reason=cmd.reason,
    )

    event = domains.LoyaltyPointsAdded(
        points=cmd.points,
        new_balance=new_balance,
        reason=cmd.reason,
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )
