"""Fulfillment Saga gRPC server.

Creates shipments when orders complete.
Supports both TCP and Unix Domain Socket (UDS) transports.
"""

import os
import sys
from pathlib import Path

import grpc
import structlog
from google.protobuf.any_pb2 import Any

# Add common to path for server utilities
sys.path.insert(0, str(Path(__file__).parent.parent / "common"))

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

SAGA_NAME = "fulfillment"
SOURCE_DOMAIN = "order"
TARGET_DOMAIN = "fulfillment"


def process_events(event_book: angzarr.EventBook) -> list[angzarr.CommandBook]:
    if not event_book.pages:
        return []

    commands = []

    for page in event_book.pages:
        if not page.event:
            continue

        if not page.event.type_url.endswith("OrderCompleted"):
            continue

        # Verify it decodes
        event = domains.OrderCompleted()
        page.event.Unpack(event)

        # Get order ID from root
        order_id = ""
        if event_book.cover and event_book.cover.root:
            order_id = event_book.cover.root.value.hex()

        if not order_id:
            continue

        # Create shipment command
        cmd = domains.CreateShipment(order_id=order_id)
        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.examples/")

        cmd_book = angzarr.CommandBook(
            cover=angzarr.Cover(domain=TARGET_DOMAIN, root=event_book.cover.root),
            pages=[angzarr.CommandPage(sequence=0, sync_mode=angzarr.SYNC_MODE_NONE, command=cmd_any)],
            correlation_id=event_book.correlation_id,
        )

        commands.append(cmd_book)

    return commands


class SagaServicer(angzarr_pb2_grpc.SagaServicer):
    def __init__(self) -> None:
        self.log = logger.bind(saga=SAGA_NAME)

    def Handle(self, request: angzarr.EventBook, context: grpc.ServicerContext) -> angzarr.SagaResponse:
        commands = process_events(request)
        if commands:
            self.log.info("processed_events", commands_generated=len(commands))
        return angzarr.SagaResponse(commands=commands)


def serve() -> None:
    """Start the gRPC server.

    Supports both TCP and UDS transports based on environment variables.
    For sagas, uses SAGA_NAME env var for socket naming.
    """
    # Set saga name env var for the common server module
    os.environ.setdefault("SAGA_NAME", SAGA_NAME)

    try:
        from server import run_server
        run_server(
            angzarr_pb2_grpc.add_SagaServicer_to_server,
            SagaServicer(),
            service_name="Saga",
            domain=SAGA_NAME,
            default_port="50307",
            logger=logger,
        )
    except ImportError:
        from concurrent import futures
        from grpc_health.v1 import health, health_pb2, health_pb2_grpc

        port = os.environ.get("PORT", "50307")
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
        angzarr_pb2_grpc.add_SagaServicer_to_server(SagaServicer(), server)

        health_servicer = health.HealthServicer()
        health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
        health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

        server.add_insecure_port(f"[::]:{port}")
        logger.info("saga_server_started", saga=SAGA_NAME, port=port, source_domain=SOURCE_DOMAIN, target_domain=TARGET_DOMAIN, transport="tcp")
        server.start()
        server.wait_for_termination()


if __name__ == "__main__":
    serve()
