"""Loyalty Points Saga gRPC server.

Listens to TransactionCompleted events and sends AddLoyaltyPoints
commands to the customer domain.
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

SAGA_NAME = "loyalty_points"


def process_events(event_book: angzarr.EventBook) -> list[angzarr.CommandBook]:
    """Process events and generate commands for loyalty points."""
    if not event_book or not event_book.pages:
        return []

    commands = []

    for page in event_book.pages:
        if not page.event:
            continue

        # Check if this is a TransactionCompleted event
        if not page.event.type_url.endswith("TransactionCompleted"):
            continue

        event = domains.TransactionCompleted()
        page.event.Unpack(event)

        points = event.loyalty_points_earned
        if points <= 0:
            continue

        # Get customer_id from the transaction cover (root is the transaction ID)
        customer_id = event_book.cover.root
        if not customer_id or not customer_id.value:
            logger.warning("transaction has no root ID, skipping loyalty points")
            continue

        transaction_id = customer_id.value.hex()
        short_id = transaction_id[:16] if len(transaction_id) > 16 else transaction_id

        logger.info(
            "awarding_loyalty_points",
            points=points,
            transaction_id=short_id,
        )

        # Create AddLoyaltyPoints command
        add_points_cmd = domains.AddLoyaltyPoints(
            points=points,
            reason=f"transaction:{transaction_id}",
        )

        cmd_any = Any()
        cmd_any.Pack(add_points_cmd, type_url_prefix="type.examples/")

        command_book = angzarr.CommandBook(
            cover=angzarr.Cover(
                domain="customer",
                root=customer_id,
            ),
            pages=[
                angzarr.CommandPage(
                    sequence=0,
                    synchronous=False,
                    command=cmd_any,
                )
            ],
        )

        commands.append(command_book)

    return commands


class SagaServicer(angzarr_pb2_grpc.SagaServicer):
    """gRPC service implementation for Loyalty Points saga."""

    def __init__(self) -> None:
        self.log = logger.bind(saga=SAGA_NAME, service="saga")

    def Handle(
        self,
        request: angzarr.EventBook,
        context: grpc.ServicerContext,
    ) -> None:
        """Process events asynchronously (fire-and-forget)."""
        self.HandleSync(request, context)
        return None

    def HandleSync(
        self,
        request: angzarr.EventBook,
        context: grpc.ServicerContext,
    ) -> angzarr.SagaResponse:
        """Process events and return commands synchronously."""
        command_books = process_events(request)

        return angzarr.SagaResponse(
            commands=command_books,
        )


def serve() -> None:
    """Start the gRPC server."""
    port = os.environ.get("PORT", "50054")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_SagaServicer_to_server(SagaServicer(), server)

    # Register gRPC health service
    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info(
        "server_started",
        saga=SAGA_NAME,
        port=port,
        listens_to="transaction domain",
    )

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
