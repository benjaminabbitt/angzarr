"""Handler for CreateCustomer command."""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from handlers import CommandRejectedError


def handle_create_customer(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.CustomerState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle CreateCustomer command."""
    if state.name:
        raise CommandRejectedError("Customer already exists")

    cmd = domains.CreateCustomer()
    command_any.Unpack(cmd)

    if not cmd.name:
        raise CommandRejectedError("Customer name is required")
    if not cmd.email:
        raise CommandRejectedError("Customer email is required")

    log.info("creating_customer", name=cmd.name, email=cmd.email)

    event = domains.CustomerCreated(
        name=cmd.name,
        email=cmd.email,
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
