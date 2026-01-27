"""Fulfillment bounded context gRPC server.

Handles shipment lifecycle (pick, pack, ship, deliver).
Supports both TCP and Unix Domain Socket (UDS) transports.
"""

import sys
from pathlib import Path

import grpc
import structlog

# Add common to path for server utilities
sys.path.insert(0, str(Path(__file__).parent.parent / "common"))

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc

from handlers.state import rebuild_state, next_sequence
from handlers import (
    CommandRejectedError,
    errmsg,
    handle_create_shipment,
    handle_mark_picked,
    handle_mark_packed,
    handle_ship,
    handle_record_delivery,
)

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

DOMAIN = "fulfillment"


class AggregateServicer(angzarr_pb2_grpc.AggregateServicer):
    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(self, request: angzarr.ContextualCommand, context: grpc.ServicerContext) -> angzarr.EventBook:
        command_book = request.command
        prior_events = request.events if request.HasField("events") else None

        if not command_book.pages:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, errmsg.NO_COMMAND_PAGES)

        command_page = command_book.pages[0]
        command_any = command_page.command

        state = rebuild_state(prior_events)
        seq = next_sequence(prior_events)

        log = self.log.bind(command_type=command_any.type_url.split(".")[-1])

        try:
            if command_any.type_url.endswith("CreateShipment"):
                return handle_create_shipment(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("MarkPicked"):
                return handle_mark_picked(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("MarkPacked"):
                return handle_mark_packed(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("Ship"):
                return handle_ship(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("RecordDelivery"):
                return handle_record_delivery(command_book, command_any, state, seq, log)
            else:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, f"{errmsg.UNKNOWN_COMMAND}: {command_any.type_url}")
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    """Start the gRPC server.

    Supports both TCP and UDS transports based on environment variables.
    """
    try:
        from server import run_server
        run_server(
            angzarr_pb2_grpc.add_AggregateServicer_to_server,
            AggregateServicer(),
            service_name="Aggregate",
            domain=DOMAIN,
            default_port="50305",
            logger=logger,
        )
    except ImportError:
        import os
        from concurrent import futures
        from grpc_health.v1 import health, health_pb2, health_pb2_grpc

        port = os.environ.get("PORT", "50305")
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
        angzarr_pb2_grpc.add_AggregateServicer_to_server(AggregateServicer(), server)

        health_servicer = health.HealthServicer()
        health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
        health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

        server.add_insecure_port(f"[::]:{port}")
        logger.info("server_started", domain=DOMAIN, port=port, transport="tcp")
        server.start()
        server.wait_for_termination()


if __name__ == "__main__":
    serve()
