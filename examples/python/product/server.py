"""Product bounded context gRPC server.

Handles product catalog management.
"""

import os
from concurrent import futures

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc
from product_logic import (
    CommandRejectedError,
    handle_create_product,
    handle_discontinue,
    handle_set_price,
    handle_update_product,
)
from state import next_sequence, rebuild_state

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

DOMAIN = "product"


class BusinessLogicServicer(angzarr_pb2_grpc.BusinessLogicServicer):
    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(
        self,
        request: angzarr.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> angzarr.EventBook:
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
            if command_any.type_url.endswith("CreateProduct"):
                return handle_create_product(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("UpdateProduct"):
                return handle_update_product(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("SetPrice"):
                return handle_set_price(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("Discontinue"):
                return handle_discontinue(command_book, command_any, state, seq, log)
            else:
                context.abort(
                    grpc.StatusCode.INVALID_ARGUMENT,
                    f"Unknown command type: {command_any.type_url}",
                )
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    port = os.environ.get("PORT", "50301")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_BusinessLogicServicer_to_server(BusinessLogicServicer(), server)

    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("server_started", domain=DOMAIN, port=port)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
