"""AggregateHandler: gRPC Aggregate servicer backed by a CommandRouter.

Maps CommandRouter dispatch to the gRPC Aggregate service interface,
translating domain errors to appropriate gRPC status codes.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import grpc

from .proto.angzarr import aggregate_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .errors import CommandRejectedError
from .router import COMPONENT_AGGREGATE, CommandRouter, Descriptor
from .server import run_server

if TYPE_CHECKING:
    import structlog


class AggregateHandler(aggregate_pb2_grpc.AggregateServiceServicer):
    """gRPC Aggregate servicer backed by a CommandRouter.

    Delegates command dispatch to the router and maps domain errors
    to gRPC status codes:
    - CommandRejectedError -> FAILED_PRECONDITION
    - ValueError -> INVALID_ARGUMENT
    """

    def __init__(self, router: CommandRouter) -> None:
        self._router = router

    def GetDescriptor(
        self,
        request: types.GetDescriptorRequest,
        context: grpc.ServicerContext,
    ) -> types.ComponentDescriptor:
        """Return component descriptor for service discovery."""
        desc = self._router.descriptor()
        return types.ComponentDescriptor(
            name=desc.name,
            component_type=COMPONENT_AGGREGATE,
            inputs=[
                types.Target(domain=inp.domain, types=inp.types)
                for inp in desc.inputs
            ],
        )

    def Handle(
        self,
        request: types.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> types.BusinessResponse:
        return self._dispatch(request, context)

    def HandleSync(
        self,
        request: types.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> types.BusinessResponse:
        return self._dispatch(request, context)

    def _dispatch(
        self,
        request: types.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> types.BusinessResponse:
        try:
            return self._router.dispatch(request)
        except CommandRejectedError as e:
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))
        except ValueError as e:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(e))

    def descriptor(self) -> Descriptor:
        """Return the aggregate's component descriptor."""
        return self._router.descriptor()


def run_aggregate_server(
    domain: str,
    default_port: str,
    router: CommandRouter,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for an aggregate.

    Args:
        domain: The aggregate's domain name.
        default_port: Default TCP port if PORT env not set.
        router: CommandRouter with registered handlers.
        logger: Optional structlog logger.
    """
    handler = AggregateHandler(router)
    run_server(
        aggregate_pb2_grpc.add_AggregateServiceServicer_to_server,
        handler,
        service_name="Aggregate",
        domain=domain,
        default_port=default_port,
        logger=logger,
    )
