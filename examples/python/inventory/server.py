"""Inventory bounded context gRPC server.

Handles stock levels and reservations.
"""

import os
from concurrent import futures
from datetime import datetime, timezone
from dataclasses import dataclass, field

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc
from proto import domains_pb2 as domains

structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.make_filtering_bound_logger(0),
    context_class=dict,
    logger_factory=structlog.PrintLoggerFactory(),
)

logger = structlog.get_logger()

DOMAIN = "inventory"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


@dataclass
class InventoryState:
    product_id: str = ""
    on_hand: int = 0
    reserved: int = 0
    low_stock_threshold: int = 0
    reservations: dict = field(default_factory=dict)  # order_id -> quantity

    def exists(self) -> bool:
        return bool(self.product_id)

    def available(self) -> int:
        return self.on_hand - self.reserved

    def is_low_stock(self) -> bool:
        return self.available() < self.low_stock_threshold


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> InventoryState:
    state = InventoryState()

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("StockInitialized"):
            event = domains.StockInitialized()
            page.event.Unpack(event)
            state.product_id = event.product_id
            state.on_hand = event.quantity
            state.low_stock_threshold = event.low_stock_threshold

        elif page.event.type_url.endswith("StockReceived"):
            event = domains.StockReceived()
            page.event.Unpack(event)
            state.on_hand = event.new_on_hand

        elif page.event.type_url.endswith("StockReserved"):
            event = domains.StockReserved()
            page.event.Unpack(event)
            state.reserved += event.quantity
            state.reservations[event.order_id] = event.quantity

        elif page.event.type_url.endswith("ReservationReleased"):
            event = domains.ReservationReleased()
            page.event.Unpack(event)
            qty = state.reservations.pop(event.order_id, 0)
            state.reserved -= qty

        elif page.event.type_url.endswith("ReservationCommitted"):
            event = domains.ReservationCommitted()
            page.event.Unpack(event)
            qty = state.reservations.pop(event.order_id, 0)
            state.on_hand -= qty
            state.reserved -= qty

    return state


def handle_initialize_stock(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_receive_stock(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_reserve_stock(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_release_reservation(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_commit_reservation(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


class BusinessLogicServicer(angzarr_pb2_grpc.BusinessLogicServicer):
    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(self, request: angzarr.ContextualCommand, context: grpc.ServicerContext) -> angzarr.EventBook:
        command_book = request.command
        prior_events = request.events if request.HasField("events") else None

        if not command_book.pages:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "CommandBook has no pages")

        command_page = command_book.pages[0]
        command_any = command_page.command

        state = rebuild_state(prior_events)
        seq = next_sequence(prior_events)

        log = self.log.bind(command_type=command_any.type_url.split(".")[-1])

        try:
            if command_any.type_url.endswith("InitializeStock"):
                return handle_initialize_stock(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ReceiveStock"):
                return handle_receive_stock(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ReserveStock"):
                return handle_reserve_stock(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ReleaseReservation"):
                return handle_release_reservation(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("CommitReservation"):
                return handle_commit_reservation(command_book, command_any, state, seq, log)
            else:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, f"Unknown command type: {command_any.type_url}")
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    port = os.environ.get("PORT", "50304")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_BusinessLogicServicer_to_server(BusinessLogicServicer(), server)

    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("server_started", domain=DOMAIN, port=port)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
