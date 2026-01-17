"""Inventory command handlers.

Business logic for processing inventory commands.
"""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import InventoryState


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


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


def handle_receive_stock(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.ReceiveStock()
    command_any.Unpack(cmd)

    if cmd.quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    log.info("receiving_stock", quantity=cmd.quantity, reference=cmd.reference)

    event = domains.StockReceived(
        quantity=cmd.quantity,
        new_on_hand=state.on_hand + cmd.quantity,
        reference=cmd.reference,
        received_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_reserve_stock(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
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
        raise CommandRejectedError(f"Insufficient stock: available {state.available()}, requested {cmd.quantity}")

    log.info("reserving_stock", quantity=cmd.quantity, order_id=cmd.order_id)

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
    pages.append(angzarr.EventPage(num=seq, event=reserved_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))))

    # Check for low stock alert
    if new_available < state.low_stock_threshold and state.available() >= state.low_stock_threshold:
        alert_event = domains.LowStockAlert(
            product_id=state.product_id,
            available=new_available,
            threshold=state.low_stock_threshold,
            alerted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        )
        alert_any = Any()
        alert_any.Pack(alert_event, type_url_prefix="type.examples/")
        pages.append(angzarr.EventPage(num=seq + 1, event=alert_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))))

    return angzarr.EventBook(cover=command_book.cover, pages=pages)


def handle_release_reservation(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.ReleaseReservation()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")
    if cmd.order_id not in state.reservations:
        raise CommandRejectedError("No reservation found for this order")

    qty = state.reservations[cmd.order_id]

    log.info("releasing_reservation", order_id=cmd.order_id, quantity=qty)

    event = domains.ReservationReleased(
        order_id=cmd.order_id,
        quantity=qty,
        new_available=state.available() + qty,
        released_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_commit_reservation(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.CommitReservation()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")
    if cmd.order_id not in state.reservations:
        raise CommandRejectedError("No reservation found for this order")

    qty = state.reservations[cmd.order_id]

    log.info("committing_reservation", order_id=cmd.order_id, quantity=qty)

    event = domains.ReservationCommitted(
        order_id=cmd.order_id,
        quantity=qty,
        new_on_hand=state.on_hand - qty,
        committed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
