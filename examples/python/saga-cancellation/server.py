"""Cancellation Saga gRPC server.

Handles compensation when orders are cancelled.
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


def process_events(event_book: angzarr.EventBook) -> list[angzarr.CommandBook]:
    if not event_book.pages:
        return []

    commands = []

    for page in event_book.pages:
        if not page.event:
            continue

        if not page.event.type_url.endswith("OrderCancelled"):
            continue

        event = domains.OrderCancelled()
        page.event.Unpack(event)

        # Get order ID from root
        order_id = ""
        if event_book.cover and event_book.cover.root:
            order_id = event_book.cover.root.value.hex()

        if not order_id:
            continue

        # Release inventory reservation
        release_cmd = domains.ReleaseReservation(order_id=order_id)
        release_any = Any()
        release_any.Pack(release_cmd, type_url_prefix="type.examples/")

        release_book = angzarr.CommandBook(
            cover=angzarr.Cover(domain="inventory", root=event_book.cover.root),
            pages=[angzarr.CommandPage(sequence=0, sync_mode=angzarr.SYNC_MODE_NONE, command=release_any)],
            correlation_id=event_book.correlation_id,
        )

        commands.append(release_book)

        # If loyalty points were used, return them
        if event.loyalty_points_used > 0:
            add_points_cmd = domains.AddLoyaltyPoints(
                points=event.loyalty_points_used,
                reason="Order cancellation refund",
            )
            add_points_any = Any()
            add_points_any.Pack(add_points_cmd, type_url_prefix="type.examples/")

            # Note: In a real system, we'd need customer ID from context
            add_points_book = angzarr.CommandBook(
                cover=angzarr.Cover(domain="customer"),
                pages=[angzarr.CommandPage(sequence=0, sync_mode=angzarr.SYNC_MODE_NONE, command=add_points_any)],
                correlation_id=event_book.correlation_id,
            )

            commands.append(add_points_book)

    return commands


class SagaServicer(angzarr_pb2_grpc.SagaServicer):
    def __init__(self) -> None:
        self.log = logger.bind(saga=SAGA_NAME)

    def Handle(self, request: angzarr.EventBook, context: grpc.ServicerContext) -> angzarr.SagaResponse:
        commands = process_events(request)
        if commands:
            self.log.info("processed_cancellation", compensation_commands=len(commands))
        return angzarr.SagaResponse(commands=commands)


def serve() -> None:
    port = os.environ.get("PORT", "50309")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_SagaServicer_to_server(SagaServicer(), server)

    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("saga_server_started", saga=SAGA_NAME, port=port, source_domain=SOURCE_DOMAIN)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
