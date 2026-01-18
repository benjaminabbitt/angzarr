"""SubmitPayment command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from .state import OrderState

from .exceptions import CommandRejectedError


def handle_submit_payment(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")

    cmd = domains.SubmitPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_method:
        raise CommandRejectedError("Payment method is required")
    expected_total = state.total_after_discount()
    if cmd.amount_cents != expected_total:
        raise CommandRejectedError("Payment amount must match order total")

    log.info("submitting_payment", method=cmd.payment_method, amount=cmd.amount_cents)

    event = domains.PaymentSubmitted(
        payment_method=cmd.payment_method,
        amount_cents=cmd.amount_cents,
        submitted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
