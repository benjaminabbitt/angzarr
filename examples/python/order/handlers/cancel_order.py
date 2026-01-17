"""CancelOrder command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import OrderState

from .exceptions import CommandRejectedError


def handle_cancel_order(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if state.is_completed():
        raise CommandRejectedError("Cannot cancel completed order")
    if state.is_cancelled():
        raise CommandRejectedError("Order already cancelled")

    cmd = domains.CancelOrder()
    command_any.Unpack(cmd)

    if not cmd.reason:
        raise CommandRejectedError("Cancellation reason is required")

    log.info("cancelling_order", reason=cmd.reason)

    event = domains.OrderCancelled(
        reason=cmd.reason,
        cancelled_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        loyalty_points_used=state.loyalty_points_used,
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
