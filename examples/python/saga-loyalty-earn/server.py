"""Loyalty Earn Saga gRPC server.

Awards loyalty points when orders complete.
Two-phase protocol: Prepare declares destinations, Execute produces commands.
"""

import os
import sys
from pathlib import Path

import grpc
import structlog
from google.protobuf.any_pb2 import Any

# Add common to path for server utilities
sys.path.insert(0, str(Path(__file__).parent.parent / "common"))

from angzarr import saga_pb2 as saga
from angzarr import saga_pb2_grpc
from angzarr import types_pb2 as types
from proto import order_pb2 as order
from proto import customer_pb2 as customer

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

SAGA_NAME = "loyalty-earn"
SOURCE_DOMAIN = "order"
TARGET_DOMAIN = "customer"


def prepare(source: types.EventBook) -> list[types.Cover]:
    """Phase 1: Declare which destination aggregates are needed.

    Returns customer cover for optimistic concurrency.
    """
    if not source or not source.pages:
        return []

    if not source.cover or not source.cover.root:
        return []

    # Check if any page has OrderCompleted with points to award
    for page in source.pages:
        if not page.event:
            continue

        if not page.event.type_url.endswith("OrderCompleted"):
            continue

        event = order.OrderCompleted()
        page.event.Unpack(event)

        if event.loyalty_points_earned > 0:
            # Request customer aggregate state (same root as order)
            return [types.Cover(domain=TARGET_DOMAIN, root=source.cover.root)]

    return []


def execute(source: types.EventBook, destinations: list[types.EventBook]) -> list[types.CommandBook]:
    """Phase 2: Produce AddLoyaltyPoints commands given source and destination state."""
    if not source or not source.pages:
        return []

    # Calculate target sequence from destination state
    target_sequence = 0
    if destinations and destinations[0]:
        target_sequence = len(destinations[0].pages)

    commands = []

    for page in source.pages:
        if not page.event:
            continue

        if not page.event.type_url.endswith("OrderCompleted"):
            continue

        event = order.OrderCompleted()
        page.event.Unpack(event)

        # Skip if no points to award
        if event.loyalty_points_earned <= 0:
            continue

        if not source.cover or not source.cover.root:
            continue

        # Create AddLoyaltyPoints command
        cmd = customer.AddLoyaltyPoints(
            points=event.loyalty_points_earned,
            reason=f"order:{source.correlation_id}",
        )
        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.examples/")

        cmd_book = types.CommandBook(
            cover=types.Cover(domain=TARGET_DOMAIN, root=source.cover.root),
            pages=[types.CommandPage(sequence=target_sequence, command=cmd_any)],
            correlation_id=source.correlation_id,
        )

        commands.append(cmd_book)

    return commands


class SagaServicer(saga_pb2_grpc.SagaServicer):
    def __init__(self) -> None:
        self.log = logger.bind(saga=SAGA_NAME)

    def Prepare(self, request: saga.SagaPrepareRequest, context: grpc.ServicerContext) -> saga.SagaPrepareResponse:
        """Phase 1: Declare which destination aggregates are needed."""
        destinations = prepare(request.source)
        return saga.SagaPrepareResponse(destinations=destinations)

    def Execute(self, request: saga.SagaExecuteRequest, context: grpc.ServicerContext) -> types.SagaResponse:
        """Phase 2: Produce commands given source and destination state."""
        commands = execute(request.source, list(request.destinations))
        if commands:
            self.log.info("processed_events", commands_generated=len(commands))
        return types.SagaResponse(commands=commands)

    def Retry(self, request: saga.SagaRetryRequest, context: grpc.ServicerContext) -> types.SagaResponse:
        """Phase 2 (alternate): Retry after command rejection."""
        commands = execute(request.source, list(request.destinations))
        if commands:
            self.log.info("retrying_saga", attempt=request.attempt, commands_generated=len(commands))
        return types.SagaResponse(commands=commands)


def serve() -> None:
    """Start the gRPC server."""
    os.environ.setdefault("SAGA_NAME", SAGA_NAME)

    try:
        from server import run_server
        run_server(
            saga_pb2_grpc.add_SagaServicer_to_server,
            SagaServicer(),
            service_name="Saga",
            domain=SAGA_NAME,
            default_port="50308",
            logger=logger,
        )
    except ImportError:
        from concurrent import futures
        from grpc_health.v1 import health, health_pb2, health_pb2_grpc

        port = os.environ.get("PORT", "50308")
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
        saga_pb2_grpc.add_SagaServicer_to_server(SagaServicer(), server)

        health_servicer = health.HealthServicer()
        health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
        health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

        server.add_insecure_port(f"[::]:{port}")
        logger.info("saga_server_started", saga=SAGA_NAME, port=port, source_domain=SOURCE_DOMAIN, target_domain=TARGET_DOMAIN, transport="tcp")
        server.start()
        server.wait_for_termination()


if __name__ == "__main__":
    serve()
