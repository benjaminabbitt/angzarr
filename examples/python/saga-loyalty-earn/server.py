"""Loyalty Earn Saga gRPC server.

Awards loyalty points when orders are delivered.
"""

import os
from concurrent import futures

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc

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

SAGA_NAME = "loyalty-earn"
SOURCE_DOMAIN = "fulfillment"
TARGET_DOMAIN = "customer"


class SagaServicer(angzarr_pb2_grpc.SagaServicer):
    def __init__(self) -> None:
        self.log = logger.bind(saga=SAGA_NAME)

    def Handle(self, request: angzarr.EventBook, context: grpc.ServicerContext) -> angzarr.SagaResponse:
        # Customer ID and points would come from saga context
        return angzarr.SagaResponse(commands=[])


def serve() -> None:
    port = os.environ.get("PORT", "50308")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_SagaServicer_to_server(SagaServicer(), server)

    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("saga_server_started", saga=SAGA_NAME, port=port, source_domain=SOURCE_DOMAIN, target_domain=TARGET_DOMAIN)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
