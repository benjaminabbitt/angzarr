"""Handler for ReceiveStock command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import inventory_pb2 as inventory

from handlers.state import InventoryState


def handle_receive_stock(
    command_book: types.CommandBook,
    command_any: Any,
    state: InventoryState,
    seq: int,
) -> types.EventBook:
    """Handle ReceiveStock command."""
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = inventory.ReceiveStock()
    command_any.Unpack(cmd)

    if cmd.quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    event = inventory.StockReceived(
        quantity=cmd.quantity,
        new_on_hand=state.on_hand + cmd.quantity,
        reference=cmd.reference,
        received_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
