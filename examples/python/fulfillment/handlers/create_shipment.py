"""CreateShipment command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from .state import FulfillmentState
from handlers.exceptions import CommandRejectedError, errmsg


def handle_create_shipment(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError(errmsg.SHIPMENT_EXISTS)

    cmd = domains.CreateShipment()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError(errmsg.ORDER_ID_REQUIRED)

    log.info("creating_shipment", order_id=cmd.order_id)

    event = domains.ShipmentCreated(
        order_id=cmd.order_id,
        status="pending",
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
