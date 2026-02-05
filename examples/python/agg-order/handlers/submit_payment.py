"""Handler for SubmitPayment command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import order_pb2 as order

from .state import OrderState


def handle_submit_payment(
    command_book: types.CommandBook,
    command_any: Any,
    state: OrderState,
    seq: int,
) -> types.EventBook:
    """Handle SubmitPayment command."""
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")

    cmd = order.SubmitPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_method:
        raise CommandRejectedError("Payment method is required")
    expected_total = state.total_after_discount()
    if cmd.amount_cents != expected_total:
        raise CommandRejectedError("Payment amount must match order total")

    event = order.PaymentSubmitted(
        payment_method=cmd.payment_method,
        amount_cents=cmd.amount_cents,
        submitted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
