"""Handler for ReserveStock command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains

from handlers.state import InventoryState


def handle_reserve_stock(
    command_book: types.CommandBook,
    command_any: Any,
    state: InventoryState,
    seq: int,
) -> types.EventBook:
    """Handle ReserveStock command."""
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.ReserveStock()
    command_any.Unpack(cmd)

    if cmd.quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")
    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")
    if cmd.order_id in state.reservations:
        raise CommandRejectedError("Reservation already exists for this order")
    if state.available() < cmd.quantity:
        raise CommandRejectedError(
            f"Insufficient stock: available {state.available()}, requested {cmd.quantity}"
        )

    new_available = state.available() - cmd.quantity

    pages = []

    reserved_event = domains.StockReserved(
        quantity=cmd.quantity,
        order_id=cmd.order_id,
        new_available=new_available,
        reserved_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    reserved_any = Any()
    reserved_any.Pack(reserved_event, type_url_prefix="type.examples/")
    pages.append(
        types.EventPage(
            num=seq,
            event=reserved_any,
            created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        )
    )

    if new_available < state.low_stock_threshold and state.available() >= state.low_stock_threshold:
        alert_event = domains.LowStockAlert(
            product_id=state.product_id,
            available=new_available,
            threshold=state.low_stock_threshold,
            alerted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        )
        alert_any = Any()
        alert_any.Pack(alert_event, type_url_prefix="type.examples/")
        pages.append(
            types.EventPage(
                num=seq + 1,
                event=alert_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        )

    return types.EventBook(cover=command_book.cover, pages=pages)
