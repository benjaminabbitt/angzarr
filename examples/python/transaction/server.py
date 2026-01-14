"""Transaction bounded context gRPC server.

Handles purchases, discounts, and transaction lifecycle.
"""

import os
from concurrent import futures
from datetime import datetime, timezone

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc
from proto import domains_pb2 as domains

# Configure structlog
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

DOMAIN = "transaction"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


def now() -> Timestamp:
    """Get current timestamp."""
    ts = Timestamp()
    ts.FromDatetime(datetime.now(timezone.utc))
    return ts


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    """Return the next event sequence number based on prior events."""
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> domains.TransactionState:
    """Rebuild transaction state from events."""
    state = domains.TransactionState(status="new")

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("TransactionCreated"):
            event = domains.TransactionCreated()
            page.event.Unpack(event)
            state.customer_id = event.customer_id
            state.items.extend(event.items)
            state.subtotal_cents = event.subtotal_cents
            state.status = "pending"

        elif page.event.type_url.endswith("DiscountApplied"):
            event = domains.DiscountApplied()
            page.event.Unpack(event)
            state.discount_cents = event.discount_cents
            state.discount_type = event.discount_type

        elif page.event.type_url.endswith("TransactionCompleted"):
            state.status = "completed"

        elif page.event.type_url.endswith("TransactionCancelled"):
            state.status = "cancelled"

    return state


def handle_create_transaction(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.TransactionState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle CreateTransaction command."""
    if state.status != "new":
        raise CommandRejectedError("Transaction already exists")

    cmd = domains.CreateTransaction()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("customer_id is required")
    if not cmd.items:
        raise CommandRejectedError("at least one item is required")

    subtotal = sum(item.quantity * item.unit_price_cents for item in cmd.items)

    log.info(
        "creating_transaction",
        customer_id=cmd.customer_id,
        item_count=len(cmd.items),
        subtotal_cents=subtotal,
    )

    event = domains.TransactionCreated(
        customer_id=cmd.customer_id,
        items=cmd.items,
        subtotal_cents=subtotal,
        created_at=now(),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=now())],
    )


def handle_apply_discount(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.TransactionState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle ApplyDiscount command."""
    if state.status != "pending":
        raise CommandRejectedError("Can only apply discount to pending transaction")

    cmd = domains.ApplyDiscount()
    command_any.Unpack(cmd)

    if cmd.discount_type == "percentage":
        if cmd.value < 0 or cmd.value > 100:
            raise CommandRejectedError("Percentage must be 0-100")
        discount_cents = (state.subtotal_cents * cmd.value) // 100
    elif cmd.discount_type == "fixed":
        discount_cents = min(cmd.value, state.subtotal_cents)
    elif cmd.discount_type == "coupon":
        discount_cents = 500  # $5 off
    else:
        raise CommandRejectedError(f"Unknown discount type: {cmd.discount_type}")

    log.info(
        "applying_discount",
        discount_type=cmd.discount_type,
        value=cmd.value,
        discount_cents=discount_cents,
    )

    event = domains.DiscountApplied(
        discount_type=cmd.discount_type,
        value=cmd.value,
        discount_cents=discount_cents,
        coupon_code=cmd.coupon_code,
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=now())],
    )


def handle_complete_transaction(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.TransactionState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle CompleteTransaction command."""
    if state.status != "pending":
        raise CommandRejectedError("Can only complete pending transaction")

    cmd = domains.CompleteTransaction()
    command_any.Unpack(cmd)

    final_total = max(0, state.subtotal_cents - state.discount_cents)
    loyalty_points = final_total // 100

    log.info(
        "completing_transaction",
        final_total_cents=final_total,
        payment_method=cmd.payment_method,
        loyalty_points_earned=loyalty_points,
    )

    event = domains.TransactionCompleted(
        final_total_cents=final_total,
        payment_method=cmd.payment_method,
        loyalty_points_earned=loyalty_points,
        completed_at=now(),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=now())],
    )


def handle_cancel_transaction(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.TransactionState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle CancelTransaction command."""
    if state.status != "pending":
        raise CommandRejectedError("Can only cancel pending transaction")

    cmd = domains.CancelTransaction()
    command_any.Unpack(cmd)

    log.info("cancelling_transaction", reason=cmd.reason)

    event = domains.TransactionCancelled(reason=cmd.reason, cancelled_at=now())

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=now())],
    )


class BusinessLogicServicer(angzarr_pb2_grpc.BusinessLogicServicer):
    """gRPC service implementation for Transaction business logic."""

    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(
        self,
        request: angzarr.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> angzarr.EventBook:
        """Process a command and return resulting events."""
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
            if command_any.type_url.endswith("CreateTransaction"):
                return handle_create_transaction(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("ApplyDiscount"):
                return handle_apply_discount(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("CompleteTransaction"):
                return handle_complete_transaction(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("CancelTransaction"):
                return handle_cancel_transaction(command_book, command_any, state, seq, log)
            else:
                context.abort(
                    grpc.StatusCode.INVALID_ARGUMENT,
                    f"Unknown command type: {command_any.type_url}",
                )
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    """Start the gRPC server."""
    port = os.environ.get("PORT", "50053")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_BusinessLogicServicer_to_server(BusinessLogicServicer(), server)

    # Register gRPC health service
    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("server_started", domain=DOMAIN, port=port)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
