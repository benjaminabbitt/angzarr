"""ProjectorHandler: gRPC Projector servicer.

Projectors consume events and produce read models. The handler receives
EventBooks and delegates to a user-provided function for projection logic.

Client projectors implement the simple Projector service (not ProjectorCoordinator).
The coordinators are for angzarr-internal orchestration only.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable

import grpc

from .proto.angzarr import projector_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .router import Descriptor, TargetDesc
from .server import run_server

if TYPE_CHECKING:
    import structlog

ProjectorHandleFunc = Callable[[types.EventBook], types.Projection]


class ProjectorHandler(projector_pb2_grpc.ProjectorServiceServicer):
    """gRPC Projector servicer backed by a handle function.

    Implements the simple Projector service for client projector logic.
    """

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
        """Return component descriptor for service discovery."""
        desc = self.descriptor()
        return types.ComponentDescriptor(
            name=desc.name,
            component_type=desc.component_type,
            inputs=[
                types.Target(domain=inp.domain, types=inp.types)
                for inp in desc.inputs
            ],
        )

    def Handle(
        self,
        request: types.EventBook,
        context: grpc.ServicerContext,
    ) -> types.Projection:
        """Process an EventBook and return Projection.

        Projector.Handle returns Projection with projection results.
        """
        if self._handle_fn is not None:
            return self._handle_fn(request)
        return types.Projection()

    def descriptor(self) -> Descriptor:
        """Build a component descriptor."""
        return Descriptor(
            name=self._name,
            component_type="projector",
            inputs=[TargetDesc(domain=d) for d in self._domains],
        )


def run_projector_server(
    name: str,
    default_port: str,
    handler: ProjectorHandler,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for a projector."""
    run_server(
        projector_pb2_grpc.add_ProjectorServiceServicer_to_server,
        handler,
        service_name="Projector",
        domain=name,
        default_port=default_port,
        logger=logger,
    )
