"""ProcessManagerHandler: gRPC ProcessManager servicer.

Two-phase protocol for stateful workflow coordinators:
  Prepare → declare additional destinations needed beyond the trigger
  Handle  → produce commands and process events given full context

Supports two patterns:
1. Functional: ProcessManagerHandler(name).with_prepare(...).with_handle(...)
2. OO class: ProcessManagerHandler(OrderWorkflowPM)
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable, Union

import grpc

from .process_manager import ProcessManager
from .proto.angzarr import process_manager_pb2 as pm
from .proto.angzarr import process_manager_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .server import run_server

if TYPE_CHECKING:
    import structlog

PMPrepareFunc = Callable[
    [types.EventBook, types.EventBook],
    list[types.Cover],
]
PMHandleFunc = Callable[
    [types.EventBook, types.EventBook, list[types.EventBook]],
    tuple[list[types.CommandBook], types.EventBook | None],
]


class ProcessManagerHandler(process_manager_pb2_grpc.ProcessManagerServiceServicer):
    """gRPC ProcessManager servicer.

    Process managers are stateful coordinators for long-running workflows
    across multiple aggregates. They maintain their own event-sourced state
    and react to events from multiple domains.

    Two patterns:
        ProcessManagerHandler(name).with_prepare(...).with_handle(...)  # functional
        ProcessManagerHandler(OrderWorkflowPM)  # OO class
    """

    def __init__(self, name_or_class: Union[str, type[ProcessManager]]) -> None:
        if isinstance(name_or_class, type) and issubclass(
            name_or_class, ProcessManager
        ):
            # OO pattern: ProcessManager class
            self._pm_class = name_or_class
            self._name = name_or_class.name
            self._prepare_fn: PMPrepareFunc | None = None
            self._handle_fn: PMHandleFunc | None = None
        else:
            # Functional pattern: name string
            self._pm_class = None
            self._name = name_or_class
            self._prepare_fn = None
            self._handle_fn = None

    def with_prepare(self, fn: PMPrepareFunc) -> ProcessManagerHandler:
        """Set the prepare callback."""
        self._prepare_fn = fn
        return self

    def with_handle(self, fn: PMHandleFunc) -> ProcessManagerHandler:
        """Set the handle callback."""
        self._handle_fn = fn
        return self

    def Prepare(
        self,
        request: pm.ProcessManagerPrepareRequest,
        context: grpc.ServicerContext,
    ) -> pm.ProcessManagerPrepareResponse:
        """Phase 1: Declare additional destinations needed."""
        # Custom prepare takes precedence
        if self._prepare_fn is not None:
            destinations = self._prepare_fn(request.trigger, request.process_state)
            return pm.ProcessManagerPrepareResponse(destinations=destinations)

        # OO pattern: use ProcessManager class
        if self._pm_class is not None:
            destinations = self._pm_class.prepare_destinations(
                request.trigger, request.process_state
            )
            return pm.ProcessManagerPrepareResponse(destinations=destinations)

        return pm.ProcessManagerPrepareResponse()

    def Handle(
        self,
        request: pm.ProcessManagerHandleRequest,
        context: grpc.ServicerContext,
    ) -> pm.ProcessManagerHandleResponse:
        """Phase 2: Produce commands and process events."""
        # Custom handle takes precedence
        if self._handle_fn is not None:
            commands, events = self._handle_fn(
                request.trigger,
                request.process_state,
                list(request.destinations),
            )
            return pm.ProcessManagerHandleResponse(
                commands=commands,
                process_events=events,
            )

        # OO pattern: use ProcessManager class
        if self._pm_class is not None:
            commands, events = self._pm_class.handle(
                request.trigger,
                request.process_state,
                list(request.destinations),
            )
            return pm.ProcessManagerHandleResponse(
                commands=commands,
                process_events=events,
            )

        return pm.ProcessManagerHandleResponse()


def run_process_manager_server(
    handler: ProcessManagerHandler,
    default_port: str,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for a process manager.

    Args:
        handler: ProcessManagerHandler with callbacks or PM class.
        default_port: Default TCP port if PORT env not set.
        logger: Optional structlog logger.
    """
    run_server(
        process_manager_pb2_grpc.add_ProcessManagerServiceServicer_to_server,
        handler,
        service_name="ProcessManager",
        domain=handler._name,
        default_port=default_port,
        logger=logger,
    )
