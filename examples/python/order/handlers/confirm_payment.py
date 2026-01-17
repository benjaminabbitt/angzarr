"""ConfirmPayment command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import OrderState

from .exceptions import CommandRejectedError


def handle_confirm_payment(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_payment_submitted():
        raise CommandRejectedError("Payment not submitted")

    cmd = domains.ConfirmPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_reference:
        raise CommandRejectedError("Payment reference is required")

    # 1 point per dollar
    loyalty_points_earned = state.total_after_discount() // 100

    log.info("confirming_payment", reference=cmd.payment_reference)

    event = domains.OrderCompleted(
        final_total_cents=state.total_after_discount(),
        payment_method=state.payment_method,
        payment_reference=cmd.payment_reference,
        loyalty_points_earned=loyalty_points_earned,
        completed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
