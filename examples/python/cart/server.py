"""Cart bounded context gRPC server.

Handles shopping cart lifecycle.
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

DOMAIN = "cart"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


@dataclass
class CartItem:
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int


@dataclass
class CartState:
    customer_id: str = ""
    items: dict = field(default_factory=dict)  # product_id -> CartItem
    subtotal_cents: int = 0
    coupon_code: str = ""
    discount_cents: int = 0
    status: str = ""

    def exists(self) -> bool:
        return bool(self.customer_id)

    def is_active(self) -> bool:
        return self.status == "active"


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> CartState:
    state = CartState()

    if event_book is None or not event_book.pages:
        return state

    if event_book.snapshot and event_book.snapshot.state:
        snap = domains.CartState()
        snap.ParseFromString(event_book.snapshot.state.value)
        state.customer_id = snap.customer_id
        state.subtotal_cents = snap.subtotal_cents
        state.coupon_code = snap.coupon_code
        state.discount_cents = snap.discount_cents
        state.status = snap.status
        for item in snap.items:
            state.items[item.product_id] = CartItem(
                product_id=item.product_id,
                name=item.name,
                quantity=item.quantity,
                unit_price_cents=item.unit_price_cents,
            )

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("CartCreated"):
            event = domains.CartCreated()
            page.event.Unpack(event)
            state.customer_id = event.customer_id
            state.status = "active"

        elif page.event.type_url.endswith("ItemAdded"):
            event = domains.ItemAdded()
            page.event.Unpack(event)
            state.items[event.product_id] = CartItem(
                product_id=event.product_id,
                name=event.name,
                quantity=event.quantity,
                unit_price_cents=event.unit_price_cents,
            )
            state.subtotal_cents = event.new_subtotal

        elif page.event.type_url.endswith("QuantityUpdated"):
            event = domains.QuantityUpdated()
            page.event.Unpack(event)
            if event.product_id in state.items:
                state.items[event.product_id].quantity = event.new_quantity
            state.subtotal_cents = event.new_subtotal

        elif page.event.type_url.endswith("ItemRemoved"):
            event = domains.ItemRemoved()
            page.event.Unpack(event)
            state.items.pop(event.product_id, None)
            state.subtotal_cents = event.new_subtotal

        elif page.event.type_url.endswith("CouponApplied"):
            event = domains.CouponApplied()
            page.event.Unpack(event)
            state.coupon_code = event.coupon_code
            state.discount_cents = event.discount_cents

        elif page.event.type_url.endswith("CartCleared"):
            state.items.clear()
            state.subtotal_cents = 0
            state.coupon_code = ""
            state.discount_cents = 0

        elif page.event.type_url.endswith("CartCheckedOut"):
            state.status = "checked_out"

    return state


def handle_create_cart(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Cart already exists")

    cmd = domains.CreateCart()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")

    log.info("creating_cart", customer_id=cmd.customer_id)

    event = domains.CartCreated(
        customer_id=cmd.customer_id,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_add_item(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.AddItem()
    command_any.Unpack(cmd)

    if not cmd.product_id:
        raise CommandRejectedError("Product ID is required")
    if cmd.quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    new_subtotal = state.subtotal_cents + (cmd.quantity * cmd.unit_price_cents)

    log.info("adding_item", product_id=cmd.product_id, quantity=cmd.quantity)

    event = domains.ItemAdded(
        product_id=cmd.product_id,
        name=cmd.name,
        quantity=cmd.quantity,
        unit_price_cents=cmd.unit_price_cents,
        new_subtotal=new_subtotal,
        added_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_update_quantity(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.UpdateQuantity()
    command_any.Unpack(cmd)

    if cmd.product_id not in state.items:
        raise CommandRejectedError("Item not in cart")
    if cmd.new_quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    item = state.items[cmd.product_id]
    old_subtotal = item.quantity * item.unit_price_cents
    new_item_subtotal = cmd.new_quantity * item.unit_price_cents
    new_subtotal = state.subtotal_cents - old_subtotal + new_item_subtotal

    log.info("updating_quantity", product_id=cmd.product_id, new_quantity=cmd.new_quantity)

    event = domains.QuantityUpdated(
        product_id=cmd.product_id,
        old_quantity=item.quantity,
        new_quantity=cmd.new_quantity,
        new_subtotal=new_subtotal,
        updated_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_remove_item(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.RemoveItem()
    command_any.Unpack(cmd)

    if cmd.product_id not in state.items:
        raise CommandRejectedError("Item not in cart")

    item = state.items[cmd.product_id]
    item_subtotal = item.quantity * item.unit_price_cents
    new_subtotal = state.subtotal_cents - item_subtotal

    log.info("removing_item", product_id=cmd.product_id)

    event = domains.ItemRemoved(
        product_id=cmd.product_id,
        quantity=item.quantity,
        new_subtotal=new_subtotal,
        removed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_apply_coupon(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")
    if state.coupon_code:
        raise CommandRejectedError("Coupon already applied")

    cmd = domains.ApplyCoupon()
    command_any.Unpack(cmd)

    if not cmd.code:
        raise CommandRejectedError("Coupon code is required")

    if cmd.coupon_type == "percentage":
        if cmd.value < 0 or cmd.value > 100:
            raise CommandRejectedError("Percentage must be 0-100")
        discount_cents = (state.subtotal_cents * cmd.value) // 100
    elif cmd.coupon_type == "fixed":
        if cmd.value < 0:
            raise CommandRejectedError("Fixed discount cannot be negative")
        discount_cents = min(cmd.value, state.subtotal_cents)
    else:
        raise CommandRejectedError("Invalid coupon type")

    log.info("applying_coupon", code=cmd.code, discount_cents=discount_cents)

    event = domains.CouponApplied(
        coupon_code=cmd.code,
        coupon_type=cmd.coupon_type,
        value=cmd.value,
        discount_cents=discount_cents,
        applied_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_clear_cart(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    log.info("clearing_cart")

    event = domains.CartCleared(
        new_subtotal=0,
        cleared_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_checkout(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")
    if not state.items:
        raise CommandRejectedError("Cart is empty")

    log.info("checking_out")

    event = domains.CartCheckedOut(
        final_subtotal=state.subtotal_cents,
        discount_cents=state.discount_cents,
        checked_out_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
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
            if command_any.type_url.endswith("CreateCart"):
                return handle_create_cart(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("AddItem"):
                return handle_add_item(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("UpdateQuantity"):
                return handle_update_quantity(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("RemoveItem"):
                return handle_remove_item(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ApplyCoupon"):
                return handle_apply_coupon(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ClearCart"):
                return handle_clear_cart(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("Checkout"):
                return handle_checkout(command_book, command_any, state, seq, log)
            else:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, f"Unknown command type: {command_any.type_url}")
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    port = os.environ.get("PORT", "50302")

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
