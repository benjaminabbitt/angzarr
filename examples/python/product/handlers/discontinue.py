"""Discontinue command handler."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from .exceptions import CommandRejectedError


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
