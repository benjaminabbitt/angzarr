"""CreateCart command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import CartState

from .errors import CommandRejectedError


def handle_create_cart(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Cart already exists")

    cmd = domains.CreateCart()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")

    log.info("creating_cart", customer_id=cmd.customer_id)

    event = domains.CartCreated(
        customer_id=cmd.customer_id,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
