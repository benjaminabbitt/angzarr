"""Order bounded context gRPC server.

Handles order lifecycle and payment processing.
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

DOMAIN = "order"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


@dataclass
class LineItem:
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int


@dataclass
class OrderState:
    customer_id: str = ""
    items: list = field(default_factory=list)
    subtotal_cents: int = 0
    discount_cents: int = 0
    loyalty_points_used: int = 0
    payment_method: str = ""
    payment_reference: str = ""
    status: str = ""

    def exists(self) -> bool:
        return bool(self.customer_id)

    def is_pending(self) -> bool:
        return self.status == "pending"

    def is_payment_submitted(self) -> bool:
        return self.status == "payment_submitted"

    def is_completed(self) -> bool:
        return self.status == "completed"

    def is_cancelled(self) -> bool:
        return self.status == "cancelled"

    def total_after_discount(self) -> int:
        return self.subtotal_cents - self.discount_cents


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> OrderState:
    state = OrderState()

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("OrderCreated"):
            event = domains.OrderCreated()
            page.event.Unpack(event)
            state.customer_id = event.customer_id
            state.subtotal_cents = event.subtotal_cents
            state.status = "pending"
            state.items = [
                LineItem(i.product_id, i.name, i.quantity, i.unit_price_cents)
                for i in event.items
            ]

        elif page.event.type_url.endswith("LoyaltyDiscountApplied"):
            event = domains.LoyaltyDiscountApplied()
            page.event.Unpack(event)
            state.loyalty_points_used = event.points_used
            state.discount_cents = event.discount_cents

        elif page.event.type_url.endswith("PaymentSubmitted"):
            event = domains.PaymentSubmitted()
            page.event.Unpack(event)
            state.payment_method = event.payment_method
            state.status = "payment_submitted"

        elif page.event.type_url.endswith("OrderCompleted"):
            event = domains.OrderCompleted()
            page.event.Unpack(event)
            state.payment_reference = event.payment_reference
            state.status = "completed"

        elif page.event.type_url.endswith("OrderCancelled"):
            state.status = "cancelled"

    return state


def handle_create_order(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Order already exists")

    cmd = domains.CreateOrder()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")
    if not cmd.items:
        raise CommandRejectedError("Order must have at least one item")

    subtotal = sum(item.quantity * item.unit_price_cents for item in cmd.items)

    log.info("creating_order", customer_id=cmd.customer_id, item_count=len(cmd.items))

    event = domains.OrderCreated(
        customer_id=cmd.customer_id,
        subtotal_cents=subtotal,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.items.extend(cmd.items)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_apply_loyalty_discount(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")
    if state.loyalty_points_used > 0:
        raise CommandRejectedError("Loyalty discount already applied")

    cmd = domains.ApplyLoyaltyDiscount()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")
    if cmd.discount_cents <= 0:
        raise CommandRejectedError("Discount must be positive")
    if cmd.discount_cents > state.subtotal_cents:
        raise CommandRejectedError("Discount cannot exceed subtotal")

    log.info("applying_loyalty_discount", points=cmd.points, discount_cents=cmd.discount_cents)

    event = domains.LoyaltyDiscountApplied(
        points_used=cmd.points,
        discount_cents=cmd.discount_cents,
        applied_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_submit_payment(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")

    cmd = domains.SubmitPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_method:
        raise CommandRejectedError("Payment method is required")
    expected_total = state.total_after_discount()
    if cmd.amount_cents != expected_total:
        raise CommandRejectedError("Payment amount must match order total")

    log.info("submitting_payment", method=cmd.payment_method, amount=cmd.amount_cents)

    event = domains.PaymentSubmitted(
        payment_method=cmd.payment_method,
        amount_cents=cmd.amount_cents,
        submitted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_confirm_payment(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_payment_submitted():
        raise CommandRejectedError("Payment not submitted")

    cmd = domains.ConfirmPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_reference:
        raise CommandRejectedError("Payment reference is required")

    # 1 point per dollar
    loyalty_points_earned = state.total_after_discount() // 100

    log.info("confirming_payment", reference=cmd.payment_reference)

    event = domains.OrderCompleted(
        final_total_cents=state.total_after_discount(),
        payment_method=state.payment_method,
        payment_reference=cmd.payment_reference,
        loyalty_points_earned=loyalty_points_earned,
        completed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_cancel_order(command_book, command_any, state, seq, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if state.is_completed():
        raise CommandRejectedError("Cannot cancel completed order")
    if state.is_cancelled():
        raise CommandRejectedError("Order already cancelled")

    cmd = domains.CancelOrder()
    command_any.Unpack(cmd)

    if not cmd.reason:
        raise CommandRejectedError("Cancellation reason is required")

    log.info("cancelling_order", reason=cmd.reason)

    event = domains.OrderCancelled(
        reason=cmd.reason,
        cancelled_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        loyalty_points_used=state.loyalty_points_used,
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
            if command_any.type_url.endswith("CreateOrder"):
                return handle_create_order(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ApplyLoyaltyDiscount"):
                return handle_apply_loyalty_discount(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("SubmitPayment"):
                return handle_submit_payment(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ConfirmPayment"):
                return handle_confirm_payment(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("CancelOrder"):
                return handle_cancel_order(command_book, command_any, state, seq, log)
            else:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, f"Unknown command type: {command_any.type_url}")
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    port = os.environ.get("PORT", "50303")

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
