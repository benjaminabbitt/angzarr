"""Ship command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from .state import FulfillmentState
from handlers.exceptions import CommandRejectedError, errmsg


def handle_ship(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError(errmsg.SHIPMENT_NOT_FOUND)
    if not state.is_packing():
        raise CommandRejectedError(errmsg.NOT_PACKED)

    cmd = domains.Ship()
    command_any.Unpack(cmd)

    if not cmd.carrier:
        raise CommandRejectedError(errmsg.CARRIER_REQUIRED)
    if not cmd.tracking_number:
        raise CommandRejectedError(errmsg.TRACKING_NUMBER_REQUIRED)

    log.info("shipping", carrier=cmd.carrier, tracking_number=cmd.tracking_number)

    event = domains.Shipped(
        carrier=cmd.carrier,
        tracking_number=cmd.tracking_number,
        shipped_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
