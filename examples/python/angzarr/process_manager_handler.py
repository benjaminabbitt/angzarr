"""ProcessManagerHandler: gRPC ProcessManager servicer.

Two-phase protocol for stateful workflow coordinators:
  Prepare → declare additional destinations needed beyond the trigger
  Handle  → produce commands and process events given full context
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable

import grpc

from angzarr import process_manager_pb2 as pm
from angzarr import process_manager_pb2_grpc
from angzarr import types_pb2 as types
from router import Descriptor, SubscriptionDesc
from server import run_server

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


class ProcessManagerHandler(process_manager_pb2_grpc.ProcessManagerServicer):
    """gRPC ProcessManager servicer.

    Process managers are stateful coordinators for long-running workflows
    across multiple aggregates. They maintain their own event-sourced state
    and react to events from multiple domains.
    """

    def __init__(self, name: str) -> None:
        self._name = name
        self._inputs: list[SubscriptionDesc] = []
        self._prepare_fn: PMPrepareFunc | None = None
        self._handle_fn: PMHandleFunc | None = None

    def listen_to(self, domain: str, *event_types: str) -> ProcessManagerHandler:
        """Subscribe to events from a domain."""
        self._inputs.append(SubscriptionDesc(domain=domain, event_types=list(event_types)))
        return self

    def with_prepare(self, fn: PMPrepareFunc) -> ProcessManagerHandler:
        """Set the prepare callback."""
        self._prepare_fn = fn
        return self

    def with_handle(self, fn: PMHandleFunc) -> ProcessManagerHandler:
        """Set the handle callback."""
        self._handle_fn = fn
        return self

    def GetDescriptor(
        self,
        request: types.GetDescriptorRequest,
        context: grpc.ServicerContext,
    ) -> types.ComponentDescriptor:
        """Return the component descriptor."""
        desc = self.descriptor()
        return types.ComponentDescriptor(
            name=desc.name,
            component_type=desc.component_type,
            inputs=[
                types.Subscription(domain=inp.domain, event_types=inp.event_types)
                for inp in desc.inputs
            ],
        )

    def Prepare(
        self,
        request: pm.ProcessManagerPrepareRequest,
        context: grpc.ServicerContext,
    ) -> pm.ProcessManagerPrepareResponse:
        """Phase 1: Declare additional destinations needed."""
        if self._prepare_fn is not None:
            destinations = self._prepare_fn(request.trigger, request.process_state)
            return pm.ProcessManagerPrepareResponse(destinations=destinations)
        return pm.ProcessManagerPrepareResponse()

    def Handle(
        self,
        request: pm.ProcessManagerHandleRequest,
        context: grpc.ServicerContext,
    ) -> pm.ProcessManagerHandleResponse:
        """Phase 2: Produce commands and process events."""
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
        return pm.ProcessManagerHandleResponse()

    def descriptor(self) -> Descriptor:
        """Build a component descriptor."""
        return Descriptor(
            name=self._name,
            component_type="process_manager",
            inputs=list(self._inputs),
        )


def run_process_manager_server(
    name: str,
    default_port: str,
    handler: ProcessManagerHandler,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for a process manager."""
    run_server(
        process_manager_pb2_grpc.add_ProcessManagerServicer_to_server,
        handler,
        service_name="ProcessManager",
        domain=name,
        default_port=default_port,
        logger=logger,
    )
