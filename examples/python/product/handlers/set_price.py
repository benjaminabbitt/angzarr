"""SetPrice command handler."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from .exceptions import CommandRejectedError


def handle_set_price(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle SetPrice command."""
    if not state.sku:
        raise CommandRejectedError("Product does not exist")
    if state.status == "discontinued":
        raise CommandRejectedError("Cannot change price of discontinued product")

    cmd = domains.SetPrice()
    command_any.Unpack(cmd)

    if cmd.new_price_cents < 0:
        raise CommandRejectedError("Price cannot be negative")

    log.info("setting_price", old_price=state.price_cents, new_price=cmd.new_price_cents)

    event = domains.PriceSet(
        new_price_cents=cmd.new_price_cents,
        previous_price_cents=state.price_cents,
        set_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
