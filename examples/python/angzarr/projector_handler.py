"""ProjectorHandler: gRPC Projector servicer.

Projectors consume events and produce read models. The handler receives
EventBooks and delegates to a user-provided function for projection logic.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable

import grpc

from angzarr import projector_pb2_grpc
from angzarr import types_pb2 as types
from router import Descriptor, SubscriptionDesc
from server import run_server

if TYPE_CHECKING:
    import structlog

ProjectorHandleFunc = Callable[[types.EventBook], types.Projection]


class ProjectorHandler(projector_pb2_grpc.ProjectorServicer):
    """gRPC Projector servicer backed by a handle function."""

    def __init__(self, name: str, *domains: str) -> None:
        self._name = name
        self._domains = list(domains)
        self._handle_fn: ProjectorHandleFunc | None = None

    def with_handle(self, fn: ProjectorHandleFunc) -> ProjectorHandler:
        """Set the event handling callback."""
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

    def Handle(
        self,
        request: types.EventBook,
        context: grpc.ServicerContext,
    ) -> types.Projection:
        """Process an EventBook and return a Projection."""
        if self._handle_fn is not None:
            return self._handle_fn(request)
        return types.Projection()

    def descriptor(self) -> Descriptor:
        """Build a component descriptor."""
        return Descriptor(
            name=self._name,
            component_type="projector",
            inputs=[SubscriptionDesc(domain=d) for d in self._domains],
        )


def run_projector_server(
    name: str,
    default_port: str,
    handler: ProjectorHandler,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for a projector."""
    run_server(
        projector_pb2_grpc.add_ProjectorServicer_to_server,
        handler,
        service_name="Projector",
        domain=name,
        default_port=default_port,
        logger=logger,
    )
