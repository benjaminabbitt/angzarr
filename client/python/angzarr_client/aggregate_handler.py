"""AggregateHandler: gRPC Aggregate servicer backed by an Aggregate class or CommandRouter.

Maps command dispatch to the gRPC Aggregate service interface,
translating domain errors to appropriate gRPC status codes.

Supports two patterns:
1. Aggregate class (OO approach): AggregateHandler(Player)
2. CommandRouter (function approach): AggregateHandler(router)
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable, Union

import grpc

from .aggregate import Aggregate
from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import aggregate_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .errors import CommandRejectedError
from .router import CommandRouter
from .server import run_server

if TYPE_CHECKING:
    import structlog


class AggregateHandler(aggregate_pb2_grpc.AggregateServiceServicer):
    """gRPC Aggregate servicer backed by an Aggregate class or CommandRouter.

    Delegates command dispatch to the aggregate's handle() class method
    or the router's dispatch() method, and maps domain errors to gRPC status codes:
    - CommandRejectedError -> FAILED_PRECONDITION
    - ValueError -> INVALID_ARGUMENT
    """

    def __init__(self, handler: Union[type[Aggregate], CommandRouter]) -> None:
        if isinstance(handler, type) and issubclass(handler, Aggregate):
            self._handle = handler.handle
            self._replay: Callable[[aggregate.ReplayRequest], aggregate.ReplayResponse] | None = handler.replay
            self._domain = handler.domain
        else:
            self._handle = handler.dispatch
            self._replay = None  # CommandRouter doesn't support replay
            self._domain = handler.domain

    @property
    def domain(self) -> str:
        return self._domain

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
    ) -> aggregate.BusinessResponse:
        try:
            return self._handle(request)
        except CommandRejectedError as e:
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))
        except ValueError as e:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(e))

    def Replay(
        self,
        request: aggregate.ReplayRequest,
        context: grpc.ServicerContext,
    ) -> aggregate.ReplayResponse:
        """Replay events to compute state (for conflict detection).

        Only available for Aggregate class handlers, not CommandRouter.
        """
        if self._replay is None:
            context.abort(
                grpc.StatusCode.UNIMPLEMENTED,
                "Replay not supported for CommandRouter-based aggregates",
            )
        try:
            return self._replay(request)
        except Exception as e:
            context.abort(grpc.StatusCode.INTERNAL, str(e))


def run_aggregate_server(
    handler: Union[type[Aggregate], CommandRouter],
    default_port: str,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for an aggregate.

    Args:
        handler: Either an Aggregate subclass or a CommandRouter.
        default_port: Default TCP port if PORT env not set.
        logger: Optional structlog logger.
    """
    aggregate_handler = AggregateHandler(handler)
    run_server(
        aggregate_pb2_grpc.add_AggregateServiceServicer_to_server,
        aggregate_handler,
        service_name="Aggregate",
        domain=aggregate_handler.domain,
        default_port=default_port,
        logger=logger,
    )
