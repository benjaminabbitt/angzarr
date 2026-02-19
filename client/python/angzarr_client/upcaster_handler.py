"""UpcasterHandler: gRPC Upcaster servicer.

Upcasters transform old event versions to current versions during replay.
They enable schema evolution without breaking existing event stores.

Client upcasters implement the simple Upcaster service.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable

import grpc

from .proto.angzarr import upcaster_pb2 as upcaster
from .proto.angzarr import upcaster_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .server import run_server

if TYPE_CHECKING:
    import structlog

UpcasterHandleFunc = Callable[[list[types.EventPage]], list[types.EventPage]]


class UpcasterHandler(upcaster_pb2_grpc.UpcasterServiceServicer):
    """gRPC Upcaster servicer backed by a handle function.

    Implements the Upcaster service for client upcaster logic.
    """

    def __init__(self, name: str, domain: str) -> None:
        self._name = name
        self._domain = domain
        self._handle_fn: UpcasterHandleFunc | None = None

    def with_handle(self, fn: UpcasterHandleFunc) -> UpcasterHandler:
        """Set the event transformation callback."""
        self._handle_fn = fn
        return self

    def Upcast(
        self,
        request: upcaster.UpcastRequest,
        context: grpc.ServicerContext,
    ) -> upcaster.UpcastResponse:
        """Transform events to current version.

        Returns events in same order, transformed where applicable.
        """
        events = list(request.events)

        if self._handle_fn is not None:
            events = self._handle_fn(events)

        return upcaster.UpcastResponse(events=events)


def run_upcaster_server(
    name: str,
    default_port: str,
    handler: UpcasterHandler,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for an upcaster."""
    run_server(
        upcaster_pb2_grpc.add_UpcasterServiceServicer_to_server,
        handler,
        service_name="Upcaster",
        domain=name,
        default_port=default_port,
        logger=logger,
    )
