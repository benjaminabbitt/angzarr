"""Cancellation Saga gRPC server.

Handles compensation when orders are cancelled.
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
from proto import inventory_pb2 as inventory
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

SAGA_NAME = "cancellation"
SOURCE_DOMAIN = "order"
INVENTORY_DOMAIN = "inventory"
CUSTOMER_DOMAIN = "customer"


def get_sequence_for_domain(destinations: list[types.EventBook], domain: str, root: bytes) -> int:
    """Find the sequence for a specific domain from destinations."""
    for dest in destinations:
        if not dest or not dest.cover:
            continue
        if dest.cover.domain == domain:
            if dest.cover.root and dest.cover.root.value == root:
                return len(dest.pages)
    return 0


def prepare(source: types.EventBook) -> list[types.Cover]:
    """Phase 1: Declare which destination aggregates are needed.

    Always requests inventory; requests customer if loyalty points were used.
    """
    if not source or not source.pages:
        return []

    if not source.cover or not source.cover.root:
        return []

    covers = []
    needs_customer = False

    for page in source.pages:
        if not page.event:
            continue

        if not page.event.type_url.endswith("OrderCancelled"):
            continue

        # Always need inventory for ReleaseReservation
        covers.append(types.Cover(domain=INVENTORY_DOMAIN, root=source.cover.root))

        # Check if we need customer (loyalty points refund)
        event = order.OrderCancelled()
        page.event.Unpack(event)
        if event.loyalty_points_used > 0:
            needs_customer = True

    if needs_customer:
        covers.append(types.Cover(domain=CUSTOMER_DOMAIN, root=source.cover.root))

    return covers


def execute(source: types.EventBook, destinations: list[types.EventBook]) -> list[types.CommandBook]:
    """Phase 2: Produce compensation commands given source and destination state."""
    if not source or not source.pages:
        return []

    commands = []

    for page in source.pages:
        if not page.event:
            continue

        if not page.event.type_url.endswith("OrderCancelled"):
            continue

        event = order.OrderCancelled()
        page.event.Unpack(event)

        # Get order ID from root
        if not source.cover or not source.cover.root:
            continue

        order_id = source.cover.root.value.hex()
        root_bytes = source.cover.root.value

        if not order_id:
            continue

        # Release inventory reservation
        release_cmd = inventory.ReleaseReservation(order_id=order_id)
        release_any = Any()
        release_any.Pack(release_cmd, type_url_prefix="type.examples/")

        inventory_seq = get_sequence_for_domain(destinations, INVENTORY_DOMAIN, root_bytes)

        release_book = types.CommandBook(
            cover=types.Cover(domain=INVENTORY_DOMAIN, root=source.cover.root),
            pages=[types.CommandPage(sequence=inventory_seq, command=release_any)],
            correlation_id=source.correlation_id,
        )

        commands.append(release_book)

        # If loyalty points were used, return them
        if event.loyalty_points_used > 0:
            add_points_cmd = customer.AddLoyaltyPoints(
                points=event.loyalty_points_used,
                reason="Order cancellation refund",
            )
            add_points_any = Any()
            add_points_any.Pack(add_points_cmd, type_url_prefix="type.examples/")

            customer_seq = get_sequence_for_domain(destinations, CUSTOMER_DOMAIN, root_bytes)

            add_points_book = types.CommandBook(
                cover=types.Cover(domain=CUSTOMER_DOMAIN, root=source.cover.root),
                pages=[types.CommandPage(sequence=customer_seq, command=add_points_any)],
                correlation_id=source.correlation_id,
            )

            commands.append(add_points_book)

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
            self.log.info("processed_cancellation", compensation_commands=len(commands))
        return types.SagaResponse(commands=commands)

    def Retry(self, request: saga.SagaRetryRequest, context: grpc.ServicerContext) -> types.SagaResponse:
        """Phase 2 (alternate): Retry after command rejection."""
        commands = execute(request.source, list(request.destinations))
        if commands:
            self.log.info("retrying_cancellation", attempt=request.attempt, compensation_commands=len(commands))
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
            default_port="50309",
            logger=logger,
        )
    except ImportError:
        from concurrent import futures
        from grpc_health.v1 import health, health_pb2, health_pb2_grpc

        port = os.environ.get("PORT", "50309")
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
        saga_pb2_grpc.add_SagaServicer_to_server(SagaServicer(), server)

        health_servicer = health.HealthServicer()
        health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
        health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

        server.add_insecure_port(f"[::]:{port}")
        logger.info("saga_server_started", saga=SAGA_NAME, port=port, source_domain=SOURCE_DOMAIN, transport="tcp")
        server.start()
        server.wait_for_termination()


if __name__ == "__main__":
    serve()
