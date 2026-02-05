"""Handler for MarkPacked command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import fulfillment_pb2 as fulfillment

from .state import FulfillmentState


def handle_mark_packed(
    command_book: types.CommandBook,
    command_any: Any,
    state: FulfillmentState,
    seq: int,
) -> types.EventBook:
    """Handle MarkPacked command."""
    if not state.exists():
        raise CommandRejectedError("Shipment does not exist")
    if not state.is_picking():
        raise CommandRejectedError("Shipment is not picked")

    cmd = fulfillment.MarkPacked()
    command_any.Unpack(cmd)

    if not cmd.packer_id:
        raise CommandRejectedError("Packer ID is required")

    event = fulfillment.ItemsPacked(
        packer_id=cmd.packer_id,
        packed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
