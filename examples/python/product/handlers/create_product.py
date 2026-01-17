"""CreateProduct command handler."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from .exceptions import CommandRejectedError


def handle_create_product(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle CreateProduct command."""
    if state.sku:
        raise CommandRejectedError("Product already exists")

    cmd = domains.CreateProduct()
    command_any.Unpack(cmd)

    if not cmd.sku:
        raise CommandRejectedError("SKU is required")
    if not cmd.name:
        raise CommandRejectedError("Name is required")
    if cmd.price_cents < 0:
        raise CommandRejectedError("Price cannot be negative")

    log.info("creating_product", sku=cmd.sku, name=cmd.name, price_cents=cmd.price_cents)

    event = domains.ProductCreated(
        sku=cmd.sku,
        name=cmd.name,
        description=cmd.description,
        price_cents=cmd.price_cents,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
