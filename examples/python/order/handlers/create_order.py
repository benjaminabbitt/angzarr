"""CreateOrder command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from .state import OrderState

from .exceptions import CommandRejectedError


def handle_create_order(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Order already exists")

    cmd = domains.CreateOrder()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")
    if not cmd.items:
        raise CommandRejectedError("Order must have at least one item")

    subtotal = sum(item.quantity * item.unit_price_cents for item in cmd.items)

    log.info("creating_order", customer_id=cmd.customer_id, item_count=len(cmd.items))

    event = domains.OrderCreated(
        customer_id=cmd.customer_id,
        subtotal_cents=subtotal,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.items.extend(cmd.items)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
