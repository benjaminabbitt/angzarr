"""Initialize stock command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import InventoryState
from handlers.errors import CommandRejectedError


def handle_initialize_stock(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Inventory already initialized")

    cmd = domains.InitializeStock()
    command_any.Unpack(cmd)

    if not cmd.product_id:
        raise CommandRejectedError("Product ID is required")
    if cmd.quantity < 0:
        raise CommandRejectedError("Quantity cannot be negative")
    if cmd.low_stock_threshold < 0:
        raise CommandRejectedError("Low stock threshold cannot be negative")

    log.info("initializing_stock", product_id=cmd.product_id, quantity=cmd.quantity)

    event = domains.StockInitialized(
        product_id=cmd.product_id,
        quantity=cmd.quantity,
        low_stock_threshold=cmd.low_stock_threshold,
        initialized_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
