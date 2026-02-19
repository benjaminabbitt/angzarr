"""SagaHandler: gRPC Saga servicer backed by an EventRouter or Saga class.

Two-phase protocol matching the Saga proto service:
  Prepare  → declare destination aggregates
  Execute  → produce commands given source + destination state

Supports two patterns:
1. EventRouter (functional approach): SagaHandler(router)
2. Saga class (OO approach): SagaHandler(TableHandSaga)

Simple sagas that only need EventRouter dispatch can omit WithPrepare/WithExecute.
Complex sagas override with custom callbacks.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable, Union

import grpc

from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import saga_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .router import EventRouter
from .saga import Saga
from .server import run_server

if TYPE_CHECKING:
    import structlog

PrepareFunc = Callable[[types.EventBook], list[types.Cover]]
ExecuteFunc = Callable[
    [types.EventBook, list[types.EventBook]], list[types.CommandBook]
]


class SagaHandler(saga_pb2_grpc.SagaServiceServicer):
    """gRPC Saga servicer backed by an EventRouter or Saga class.

    Two patterns:
        SagaHandler(router) - functional approach with EventRouter
        SagaHandler(MySaga) - OO approach with Saga class

    Simple mode (default): Prepare returns empty, Execute dispatches through router/class.
    Custom mode: Override with with_prepare/with_execute for destination-aware sagas.
    """

    def __init__(self, handler: Union[EventRouter, type[Saga]]) -> None:
        self._prepare: PrepareFunc | None = None
        self._execute: ExecuteFunc | None = None

        if isinstance(handler, type) and issubclass(handler, Saga):
            # OO pattern: Saga class
            self._saga_class = handler
            self._router = None
        else:
            # Functional pattern: EventRouter
            self._saga_class = None
            self._router = handler

    def with_prepare(self, fn: PrepareFunc) -> SagaHandler:
        """Set custom prepare callback for destination declaration."""
        self._prepare = fn
        return self

    def with_execute(self, fn: ExecuteFunc) -> SagaHandler:
        """Set custom execute callback for destination-aware command production."""
        self._execute = fn
        return self

    def Prepare(
        self,
        request: saga.SagaPrepareRequest,
        context: grpc.ServicerContext,
    ) -> saga.SagaPrepareResponse:
        """Phase 1: Declare which destination aggregates are needed."""
        # Custom prepare takes precedence
        if self._prepare is not None:
            destinations = self._prepare(request.source)
            return saga.SagaPrepareResponse(destinations=destinations)

        # OO pattern: use Saga class's prepare_destinations
        if self._saga_class is not None:
            destinations = self._saga_class.prepare_destinations(request.source)
            return saga.SagaPrepareResponse(destinations=destinations)

        # Functional pattern: use EventRouter's prepare_destinations
        if self._router is not None:
            destinations = self._router.prepare_destinations(request.source)
            return saga.SagaPrepareResponse(destinations=destinations)

        return saga.SagaPrepareResponse()

    def Execute(
        self,
        request: saga.SagaExecuteRequest,
        context: grpc.ServicerContext,
    ) -> saga.SagaResponse:
        """Phase 2: Produce commands given source + destination state."""
        commands = self._execute_commands(request.source, list(request.destinations))
        return saga.SagaResponse(commands=commands)

    def _execute_commands(
        self,
        source: types.EventBook,
        destinations: list[types.EventBook],
    ) -> list[types.CommandBook]:
        """Dispatch through custom execute, Saga class, or EventRouter."""
        # Custom execute takes precedence
        if self._execute is not None:
            return self._execute(source, destinations)

        # OO pattern: use Saga class's execute
        if self._saga_class is not None:
            return self._saga_class.execute(source, destinations)

        # Functional pattern: use EventRouter's dispatch
        if self._router is not None:
            return self._router.dispatch(source, destinations)

        return []


def run_saga_server(
    name: str,
    default_port: str,
    handler: SagaHandler,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for a saga.

    Args:
        name: The saga's name.
        default_port: Default TCP port if PORT env not set.
        handler: SagaHandler with router/class and optional callbacks.
        logger: Optional structlog logger.
    """
    run_server(
        saga_pb2_grpc.add_SagaServiceServicer_to_server,
        handler,
        service_name="Saga",
        domain=name,
        default_port=default_port,
        logger=logger,
    )
