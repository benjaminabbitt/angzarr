"""Common server utilities for angzarr Python examples.

Supports both TCP and Unix Domain Socket (UDS) transports for gRPC servers.
"""

import os
from concurrent import futures
from typing import Callable

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc


def configure_logging() -> None:
    """Configure structlog with JSON rendering and ISO timestamps."""
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


def get_transport_config() -> tuple[str, str]:
    """Get transport configuration from environment.

    Returns:
        Tuple of (transport_type, address)
        - For TCP: ("tcp", "[::]:{port}")
        - For UDS: ("uds", "unix://{socket_path}")

    Environment variables:
        TRANSPORT_TYPE: "tcp" (default) or "uds"
        UDS_BASE_PATH: Base directory for sockets (default: /tmp/angzarr)
        SERVICE_NAME: Service type ("business", "saga", "projector")
        DOMAIN: Domain name for aggregates
        SAGA_NAME: Saga name (used if DOMAIN not set)
        PROJECTOR_NAME: Projector name (used if DOMAIN and SAGA_NAME not set)
    """
    transport = os.environ.get("TRANSPORT_TYPE", "tcp").lower()

    if transport == "uds":
        base_path = os.environ.get("UDS_BASE_PATH", "/tmp/angzarr")
        service_name = os.environ.get("SERVICE_NAME", "business")

        # Get the qualifier from DOMAIN, SAGA_NAME, or PROJECTOR_NAME
        qualifier = (
            os.environ.get("DOMAIN")
            or os.environ.get("SAGA_NAME")
            or os.environ.get("PROJECTOR_NAME")
            or ""
        )

        # Create socket path with optional qualifier
        if qualifier:
            socket_path = f"{base_path}/{service_name}-{qualifier}.sock"
        else:
            socket_path = f"{base_path}/{service_name}.sock"

        # Ensure parent directory exists
        os.makedirs(os.path.dirname(socket_path), exist_ok=True)

        # Remove stale socket file if exists
        if os.path.exists(socket_path):
            os.remove(socket_path)

        return ("uds", f"unix:{socket_path}")

    else:
        port = os.environ.get("PORT", "50052")
        return ("tcp", f"[::]:{port}")


def create_server(
    add_servicer_func: Callable,
    servicer: object,
    service_name: str = "",
    max_workers: int = 10,
) -> tuple[grpc.Server, str]:
    """Create a gRPC server with health checking.

    Args:
        add_servicer_func: The add_*Servicer_to_server function
        servicer: The servicer instance
        service_name: Service name for health checking
        max_workers: Maximum thread pool workers

    Returns:
        Tuple of (server, address) where address includes the transport prefix
    """
    transport_type, address = get_transport_config()

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=max_workers))

    # Add the main service
    add_servicer_func(servicer, server)

    # Add health service
    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)
    if service_name:
        health_servicer.set(service_name, health_pb2.HealthCheckResponse.SERVING)

    # Add port/socket
    if transport_type == "uds":
        server.add_insecure_port(address)
    else:
        server.add_insecure_port(address)

    return server, address


def run_server(
    add_servicer_func: Callable,
    servicer: object,
    service_name: str = "",
    domain: str = "",
    default_port: str = "50052",
    logger=None,
) -> None:
    """Run a gRPC server until termination.

    Args:
        add_servicer_func: The add_*Servicer_to_server function
        servicer: The servicer instance
        service_name: Service name for logging and health checking
        domain: Domain name (for logging)
        default_port: Default TCP port if PORT env not set
        logger: Optional structlog logger
    """
    # Set default port if not specified
    if "PORT" not in os.environ:
        os.environ["PORT"] = default_port

    server, address = create_server(add_servicer_func, servicer, service_name)

    transport_type = os.environ.get("TRANSPORT_TYPE", "tcp").lower()

    if logger:
        logger.info(
            "server_started",
            service=service_name,
            domain=domain,
            transport=transport_type,
            address=address,
        )
    else:
        print(
            f"Server started: {service_name} ({domain}) on {address} ({transport_type})"
        )

    server.start()
    server.wait_for_termination()


def cleanup_socket(socket_path: str) -> None:
    """Clean up a UDS socket file.

    Args:
        socket_path: Path to the socket file to remove
    """
    if socket_path and os.path.exists(socket_path):
        try:
            os.remove(socket_path)
        except OSError:
            pass
