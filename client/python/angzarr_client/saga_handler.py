"""SagaHandler: gRPC Saga servicer backed by SingleFluentRouter or Saga class.

Sagas are stateless translators: source events → commands for target domains.
Framework handles sequence stamping and delivery.

Supports two patterns:
1. SingleFluentRouter (fluent builder): SagaHandler(router)  # .domain().on()
2. Saga class (OO approach): SagaHandler(TableHandSaga)
"""

from __future__ import annotations

from collections.abc import Callable
from typing import TYPE_CHECKING

import grpc

from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import saga_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .router import SingleFluentRouter
from .saga import Saga
from .server import run_server

if TYPE_CHECKING:
    import structlog

# HandleFunc returns SagaResponse (proto-generated type)
HandleFunc = Callable[[types.EventBook], saga.SagaResponse]


class SagaHandler(saga_pb2_grpc.SagaServiceServicer):
    """gRPC Saga servicer backed by SingleFluentRouter or Saga class.

    Two patterns:
        SagaHandler(event_router) - fluent builder with SingleFluentRouter
        SagaHandler(MySaga) - OO approach with Saga class
    """

    def __init__(self, handler: SingleFluentRouter | type[Saga]) -> None:
        self._handle: HandleFunc | None = None
        self._saga_class: type[Saga] | None = None
        self._event_router: SingleFluentRouter | None = None

        if isinstance(handler, type) and issubclass(handler, Saga):
            # OO pattern: Saga class
            self._saga_class = handler
        elif isinstance(handler, SingleFluentRouter):
            # Fluent pattern: SingleFluentRouter
            self._event_router = handler
        else:
            # Assume it's a router-like object (duck typing for backwards compat)
            if hasattr(handler, "dispatch"):
                self._event_router = handler

    def with_handle(self, fn: HandleFunc) -> SagaHandler:
        """Set custom handle callback for saga execution."""
        self._handle = fn
        return self

    def Handle(
        self,
        request: saga.SagaHandleRequest,
        context: grpc.ServicerContext,
    ) -> saga.SagaResponse:
        """Handle source events and produce commands for target domains."""
        return self._handle_saga(request.source)

    def _handle_saga(
        self,
        source: types.EventBook,
    ) -> saga.SagaResponse:
        """Dispatch through custom handle, Saga class, or SingleFluentRouter."""
        # Custom handle takes precedence
        if self._handle is not None:
            return self._handle(source)

        # OO pattern: use Saga class's handle (returns SagaResponse)
        if self._saga_class is not None:
            return self._saga_class.handle(source)

        # Fluent pattern: use SingleFluentRouter's dispatch (returns SagaResponse)
        if self._event_router is not None:
            return self._event_router.dispatch(source)

        return saga.SagaResponse()


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
