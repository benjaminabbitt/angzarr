"""Transaction Log Projector gRPC server.

Logs transaction events using structured logging.
"""

import os
from concurrent import futures

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc

from google.protobuf import empty_pb2

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

PROJECTOR_NAME = "log-transaction"


def log_events(event_book: angzarr.EventBook) -> None:
    """Log all events in the event book."""
    if not event_book or not event_book.pages:
        return

    domain = "transaction"
    if event_book.cover:
        domain = event_book.cover.domain

    root_id = ""
    if event_book.cover and event_book.cover.root:
        root_id = event_book.cover.root.value.hex()
    short_id = root_id[:16] if len(root_id) > 16 else root_id

    for page in event_book.pages:
        if not page.event:
            continue

        sequence = 0
        if page.HasField("num"):
            sequence = page.num

        event_type = page.event.type_url
        if "." in event_type:
            event_type = event_type.split(".")[-1]

        # Create base log context
        event_logger = logger.bind(
            domain=domain,
            root_id=short_id,
            sequence=sequence,
            event_type=event_type,
        )

        # Log event-specific details
        log_event_details(event_logger, event_type, page.event)


def log_event_details(event_logger: structlog.BoundLogger, event_type: str, event_any) -> None:
    """Log event-specific details."""
    if event_type == "TransactionCreated":
        event = domains.TransactionCreated()
        event_any.Unpack(event)
        cust_id = event.customer_id
        if len(cust_id) > 16:
            cust_id = cust_id[:16]
        event_logger.info(
            "event",
            customer_id=cust_id,
            item_count=len(event.items),
            subtotal_cents=event.subtotal_cents,
        )

    elif event_type == "DiscountApplied":
        event = domains.DiscountApplied()
        event_any.Unpack(event)
        event_logger.info(
            "event",
            discount_type=event.discount_type,
            value=event.value,
            discount_cents=event.discount_cents,
            coupon_code=event.coupon_code,
        )

    elif event_type == "TransactionCompleted":
        event = domains.TransactionCompleted()
        event_any.Unpack(event)
        event_logger.info(
            "event",
            final_total_cents=event.final_total_cents,
            payment_method=event.payment_method,
            loyalty_points_earned=event.loyalty_points_earned,
        )

    elif event_type == "TransactionCancelled":
        event = domains.TransactionCancelled()
        event_any.Unpack(event)
        event_logger.info(
            "event",
            reason=event.reason,
        )

    else:
        event_logger.info("event", raw_bytes=len(event_any.value))


class ProjectorServicer(angzarr_pb2_grpc.ProjectorCoordinatorServicer):
    """gRPC service implementation for Transaction Log projector."""

    def __init__(self) -> None:
        self.log = logger.bind(projector=PROJECTOR_NAME, service="projector")

    def Handle(
        self,
        request: angzarr.EventBook,
        context: grpc.ServicerContext,
    ) -> empty_pb2.Empty:
        """Process events asynchronously (fire-and-forget)."""
        log_events(request)
        return empty_pb2.Empty()

    def HandleSync(
        self,
        request: angzarr.EventBook,
        context: grpc.ServicerContext,
    ) -> angzarr.Projection:
        """Process events and return projection synchronously."""
        log_events(request)
        # Log projector doesn't produce a projection
        return None


def serve() -> None:
    """Start the gRPC server."""
    port = os.environ.get("PORT", "50057")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_ProjectorCoordinatorServicer_to_server(ProjectorServicer(), server)

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
