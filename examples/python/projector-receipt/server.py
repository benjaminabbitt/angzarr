"""Receipt Projector gRPC server.

Generates human-readable receipts when transactions complete.
"""

import os
from concurrent import futures

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc
from google.protobuf.any_pb2 import Any

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

PROJECTOR_NAME = "receipt"


class TransactionState:
    """Rebuilt transaction state from events."""

    def __init__(self):
        self.customer_id: str = ""
        self.items: list = []
        self.subtotal_cents: int = 0
        self.discount_cents: int = 0
        self.discount_type: str = ""
        self.final_total_cents: int = 0
        self.payment_method: str = ""
        self.loyalty_points_earned: int = 0
        self.completed: bool = False


def rebuild_state(event_book: angzarr.EventBook) -> TransactionState:
    """Rebuild transaction state from events."""
    state = TransactionState()

    if not event_book or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("TransactionCreated"):
            event = domains.TransactionCreated()
            page.event.Unpack(event)
            state.customer_id = event.customer_id
            state.items = list(event.items)
            state.subtotal_cents = event.subtotal_cents

        elif page.event.type_url.endswith("DiscountApplied"):
            event = domains.DiscountApplied()
            page.event.Unpack(event)
            state.discount_type = event.discount_type
            state.discount_cents = event.discount_cents

        elif page.event.type_url.endswith("TransactionCompleted"):
            event = domains.TransactionCompleted()
            page.event.Unpack(event)
            state.final_total_cents = event.final_total_cents
            state.payment_method = event.payment_method
            state.loyalty_points_earned = event.loyalty_points_earned
            state.completed = True

    return state


def format_receipt(transaction_id: str, state: TransactionState) -> str:
    """Format a human-readable receipt."""
    lines = []

    short_tx_id = transaction_id[:16] if len(transaction_id) > 16 else transaction_id
    short_cust_id = state.customer_id[:16] if len(state.customer_id) > 16 else state.customer_id

    lines.append("=" * 40)
    lines.append("           RECEIPT")
    lines.append("=" * 40)
    lines.append(f"Transaction: {short_tx_id}...")
    lines.append(f"Customer: {short_cust_id}..." if state.customer_id else "Customer: N/A")
    lines.append("-" * 40)

    # Items
    for item in state.items:
        line_total = item.quantity * item.unit_price_cents
        lines.append(
            f"{item.quantity} x {item.name} @ ${item.unit_price_cents / 100:.2f} = ${line_total / 100:.2f}"
        )

    lines.append("-" * 40)
    lines.append(f"Subtotal:              ${state.subtotal_cents / 100:.2f}")

    if state.discount_cents > 0:
        lines.append(f"Discount ({state.discount_type}):       -${state.discount_cents / 100:.2f}")

    lines.append("-" * 40)
    lines.append(f"TOTAL:                 ${state.final_total_cents / 100:.2f}")
    lines.append(f"Payment: {state.payment_method}")
    lines.append("-" * 40)
    lines.append(f"Loyalty Points Earned: {state.loyalty_points_earned}")
    lines.append("=" * 40)
    lines.append("     Thank you for your purchase!")
    lines.append("=" * 40)

    return "\n".join(lines)


def project(event_book: angzarr.EventBook) -> angzarr.Projection | None:
    """Project events and generate a receipt if transaction completed."""
    state = rebuild_state(event_book)

    if not state.completed:
        return None

    transaction_id = ""
    if event_book.cover and event_book.cover.root:
        transaction_id = event_book.cover.root.value.hex()

    short_id = transaction_id[:16] if len(transaction_id) > 16 else transaction_id

    # Generate formatted receipt text
    receipt_text = format_receipt(transaction_id, state)

    logger.info(
        "generated_receipt",
        transaction_id=short_id,
        total_cents=state.final_total_cents,
        payment_method=state.payment_method,
    )

    # Create Receipt message
    receipt = domains.Receipt(
        transaction_id=transaction_id,
        customer_id=state.customer_id,
        items=state.items,
        subtotal_cents=state.subtotal_cents,
        discount_cents=state.discount_cents,
        final_total_cents=state.final_total_cents,
        payment_method=state.payment_method,
        loyalty_points_earned=state.loyalty_points_earned,
        formatted_text=receipt_text,
    )

    receipt_any = Any()
    receipt_any.Pack(receipt, type_url_prefix="type.examples/")

    # Get sequence from last page
    sequence = 0
    if event_book.pages:
        last_page = event_book.pages[-1]
        if last_page.HasField("num"):
            sequence = last_page.num

    return angzarr.Projection(
        cover=event_book.cover,
        projector=PROJECTOR_NAME,
        sequence=sequence,
        projection=receipt_any,
    )


class ProjectorServicer(angzarr_pb2_grpc.ProjectorServicer):
    """gRPC service implementation for Receipt projector."""

    def __init__(self) -> None:
        self.log = logger.bind(projector=PROJECTOR_NAME, service="projector")

    def Handle(
        self,
        request: angzarr.EventBook,
        context: grpc.ServicerContext,
    ) -> None:
        """Process events asynchronously (fire-and-forget)."""
        project(request)
        return None

    def HandleSync(
        self,
        request: angzarr.EventBook,
        context: grpc.ServicerContext,
    ) -> angzarr.Projection:
        """Process events and return projection synchronously."""
        return project(request)


def serve() -> None:
    """Start the gRPC server."""
    port = os.environ.get("PORT", "50055")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_ProjectorServicer_to_server(ProjectorServicer(), server)

    # Register gRPC health service
    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info(
        "server_started",
        projector=PROJECTOR_NAME,
        port=port,
        listens_to="transaction domain",
    )

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
