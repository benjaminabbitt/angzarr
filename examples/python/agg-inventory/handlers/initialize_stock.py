"""Handler for InitializeStock command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import inventory_pb2 as inventory

from handlers.state import InventoryState


def handle_initialize_stock(
    command_book: types.CommandBook,
    command_any: Any,
    state: InventoryState,
    seq: int,
) -> types.EventBook:
    """Handle InitializeStock command."""
    if state.exists():
        raise CommandRejectedError("Inventory already initialized")

    cmd = inventory.InitializeStock()
    command_any.Unpack(cmd)

    if not cmd.product_id:
        raise CommandRejectedError("Product ID is required")
    if cmd.quantity < 0:
        raise CommandRejectedError("Quantity cannot be negative")
    if cmd.low_stock_threshold < 0:
        raise CommandRejectedError("Low stock threshold cannot be negative")

    event = inventory.StockInitialized(
        product_id=cmd.product_id,
        quantity=cmd.quantity,
        low_stock_threshold=cmd.low_stock_threshold,
        initialized_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
