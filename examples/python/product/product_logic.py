"""Product command handlers and business logic."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


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


def handle_update_product(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle UpdateProduct command."""
    if not state.sku:
        raise CommandRejectedError("Product does not exist")
    if state.status == "discontinued":
        raise CommandRejectedError("Cannot update discontinued product")

    cmd = domains.UpdateProduct()
    command_any.Unpack(cmd)

    if not cmd.name:
        raise CommandRejectedError("Name is required")

    log.info("updating_product", name=cmd.name)

    event = domains.ProductUpdated(
        name=cmd.name,
        description=cmd.description,
        updated_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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


def handle_discontinue(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle Discontinue command."""
    if not state.sku:
        raise CommandRejectedError("Product does not exist")
    if state.status == "discontinued":
        raise CommandRejectedError("Product already discontinued")

    cmd = domains.Discontinue()
    command_any.Unpack(cmd)

    log.info("discontinuing_product", reason=cmd.reason)

    event = domains.ProductDiscontinued(
        reason=cmd.reason,
        discontinued_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
