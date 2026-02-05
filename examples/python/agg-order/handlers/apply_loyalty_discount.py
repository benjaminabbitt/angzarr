"""Handler for ApplyLoyaltyDiscount command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains

from .state import OrderState


def handle_apply_loyalty_discount(
    command_book: types.CommandBook,
    command_any: Any,
    state: OrderState,
    seq: int,
) -> types.EventBook:
    """Handle ApplyLoyaltyDiscount command."""
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

    event = domains.LoyaltyDiscountApplied(
        points_used=cmd.points,
        discount_cents=cmd.discount_cents,
        applied_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )
