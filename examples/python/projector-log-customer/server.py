"""Customer Log Projector gRPC server.

Logs customer events using structured logging.
"""

import os
from concurrent import futures
from datetime import datetime, timezone

import grpc
import structlog
from google.protobuf.timestamp_pb2 import Timestamp

from google.protobuf import empty_pb2

from evented import evented_pb2 as evented
from evented import evented_pb2_grpc
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

PROJECTOR_NAME = "log-customer"


def format_timestamp(ts: Timestamp) -> str:
    """Format a protobuf Timestamp as RFC 3339."""
    dt = datetime.fromtimestamp(ts.seconds + ts.nanos / 1e9, tz=timezone.utc)
    return dt.isoformat()


def log_events(event_book: evented.EventBook) -> None:
    """Log all events in the event book."""
    if not event_book or not event_book.pages:
        return

    domain = "customer"
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
    if event_type == "CustomerCreated":
        event = domains.CustomerCreated()
        event_any.Unpack(event)
        extra = {}
        if event.HasField("created_at"):
            extra["created_at"] = format_timestamp(event.created_at)
        event_logger.info(
            "event",
            name=event.name,
            email=event.email,
            **extra,
        )

    elif event_type == "LoyaltyPointsAdded":
        event = domains.LoyaltyPointsAdded()
        event_any.Unpack(event)
        event_logger.info(
            "event",
            points=event.points,
            new_balance=event.new_balance,
            reason=event.reason,
        )

    elif event_type == "LoyaltyPointsRedeemed":
        event = domains.LoyaltyPointsRedeemed()
        event_any.Unpack(event)
        event_logger.info(
            "event",
            points=event.points,
            new_balance=event.new_balance,
            redemption_type=event.redemption_type,
        )

    else:
        event_logger.info("event", raw_bytes=len(event_any.value))


class ProjectorServicer(evented_pb2_grpc.ProjectorCoordinatorServicer):
    """gRPC service implementation for Customer Log projector."""

    def __init__(self) -> None:
        self.log = logger.bind(projector=PROJECTOR_NAME, service="projector")

    def Handle(
        self,
        request: evented.EventBook,
        context: grpc.ServicerContext,
    ) -> empty_pb2.Empty:
        """Process events asynchronously (fire-and-forget)."""
        log_events(request)
        return empty_pb2.Empty()

    def HandleSync(
        self,
        request: evented.EventBook,
        context: grpc.ServicerContext,
    ) -> evented.Projection:
        """Process events and return projection synchronously."""
        log_events(request)
        # Log projector doesn't produce a projection
        return None


def serve() -> None:
    """Start the gRPC server."""
    port = os.environ.get("PORT", "50056")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    evented_pb2_grpc.add_ProjectorCoordinatorServicer_to_server(ProjectorServicer(), server)
    server.add_insecure_port(f"[::]:{port}")

    logger.info(
        "server_started",
        projector=PROJECTOR_NAME,
        port=port,
        listens_to="customer domain",
    )

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
