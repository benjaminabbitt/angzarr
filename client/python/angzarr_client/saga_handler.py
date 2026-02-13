"""SagaHandler: gRPC Saga servicer backed by an EventRouter.

Two-phase protocol matching the Saga proto service:
  Prepare  → declare destination aggregates
  Execute  → produce commands given source + destination state

Simple sagas that only need EventRouter dispatch can omit WithPrepare/WithExecute.
Complex sagas override with custom callbacks.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable

import grpc

from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import saga_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .router import COMPONENT_SAGA, Descriptor, EventRouter
from .server import run_server

if TYPE_CHECKING:
    import structlog

PrepareFunc = Callable[[types.EventBook], list[types.Cover]]
ExecuteFunc = Callable[
    [types.EventBook, list[types.EventBook]], list[types.CommandBook]
]


class SagaHandler(saga_pb2_grpc.SagaServiceServicer):
    """gRPC Saga servicer backed by an EventRouter.

    Simple mode (default): Prepare returns empty, Execute dispatches through router.
    Custom mode: Override with WithPrepare/WithExecute for destination-aware sagas.
    """

    def __init__(self, router: EventRouter) -> None:
        self._router = router
        self._prepare: PrepareFunc | None = None
        self._execute: ExecuteFunc | None = None

    def with_prepare(self, fn: PrepareFunc) -> SagaHandler:
        """Set custom prepare callback for destination declaration."""
        self._prepare = fn
        return self

    def with_execute(self, fn: ExecuteFunc) -> SagaHandler:
        """Set custom execute callback for destination-aware command production."""
        self._execute = fn
        return self

    def GetDescriptor(
        self,
        request: types.GetDescriptorRequest,
        context: grpc.ServicerContext,
    ) -> types.ComponentDescriptor:
        """Return component descriptor for service discovery."""
        desc = self._router.descriptor()
        return types.ComponentDescriptor(
            name=desc.name,
            component_type=COMPONENT_SAGA,
            inputs=[
                types.Target(domain=inp.domain, types=inp.types)
                for inp in desc.inputs
            ],
        )

    def Prepare(
        self,
        request: saga.SagaPrepareRequest,
        context: grpc.ServicerContext,
    ) -> saga.SagaPrepareResponse:
        """Phase 1: Declare which destination aggregates are needed."""
        if self._prepare is not None:
            destinations = self._prepare(request.source)
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
        """Dispatch through custom execute or fall back to router."""
        if self._execute is not None:
            return self._execute(source, destinations)
        return self._router.dispatch(source)

    def descriptor(self) -> Descriptor:
        """Return the saga's component descriptor."""
        return self._router.descriptor()


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
        handler: SagaHandler with router and optional callbacks.
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
