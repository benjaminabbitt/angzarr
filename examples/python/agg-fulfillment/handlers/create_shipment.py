"""Handler for CreateShipment command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains

from .state import FulfillmentState


def handle_create_shipment(
    command_book: types.CommandBook,
    command_any: Any,
    state: FulfillmentState,
    seq: int,
) -> types.EventBook:
    """Handle CreateShipment command."""
    if state.exists():
        raise CommandRejectedError("Shipment already exists")

    cmd = domains.CreateShipment()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")

    event = domains.ShipmentCreated(
        order_id=cmd.order_id,
        status="pending",
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )
