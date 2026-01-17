"""ApplyLoyaltyDiscount command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import OrderState

from .exceptions import CommandRejectedError


def handle_apply_loyalty_discount(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")
    if state.loyalty_points_used > 0:
        raise CommandRejectedError("Loyalty discount already applied")

    cmd = domains.ApplyLoyaltyDiscount()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")
    if cmd.discount_cents <= 0:
        raise CommandRejectedError("Discount must be positive")
    if cmd.discount_cents > state.subtotal_cents:
        raise CommandRejectedError("Discount cannot exceed subtotal")

    log.info("applying_loyalty_discount", points=cmd.points, discount_cents=cmd.discount_cents)

    event = domains.LoyaltyDiscountApplied(
        points_used=cmd.points,
        discount_cents=cmd.discount_cents,
        applied_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
