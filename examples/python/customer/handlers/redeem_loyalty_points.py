"""Handler for RedeemLoyaltyPoints command."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from handlers import CommandRejectedError


def handle_redeem_loyalty_points(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.CustomerState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle RedeemLoyaltyPoints command."""
    if not state.name:
        raise CommandRejectedError("Customer does not exist")

    cmd = domains.RedeemLoyaltyPoints()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")
    if cmd.points > state.loyalty_points:
        raise CommandRejectedError(
            f"Insufficient points: have {state.loyalty_points}, need {cmd.points}"
        )

    new_balance = state.loyalty_points - cmd.points

    log.info(
        "redeeming_loyalty_points",
        points=cmd.points,
        new_balance=new_balance,
        redemption_type=cmd.redemption_type,
    )

    event = domains.LoyaltyPointsRedeemed(
        points=cmd.points,
        new_balance=new_balance,
        redemption_type=cmd.redemption_type,
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
