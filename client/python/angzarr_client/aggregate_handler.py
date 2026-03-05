"""CommandHandlerGrpc: gRPC CommandHandler servicer backed by a CommandHandler class or CommandHandlerRouter.

Maps command dispatch to the gRPC CommandHandler service interface,
translating domain errors to appropriate gRPC status codes.

Supports two patterns:
1. CommandHandler class (OO approach): CommandHandlerGrpc(Player)
2. CommandHandlerRouter (protocol approach): CommandHandlerGrpc(router)
"""

from __future__ import annotations

from collections.abc import Callable
from typing import TYPE_CHECKING

import grpc

from .aggregate import CommandHandler
from .errors import CommandRejectedError
from .proto.angzarr import command_handler_pb2 as command_handler
from .proto.angzarr import command_handler_pb2_grpc
from .proto.angzarr import types_pb2 as types
from .router import CommandHandlerRouter
from .server import run_server
from .state_builder import CommandRouter

if TYPE_CHECKING:
    import structlog


class CommandHandlerGrpc(command_handler_pb2_grpc.CommandHandlerServiceServicer):
    """gRPC CommandHandler servicer backed by a CommandHandler class or CommandHandlerRouter.

    Delegates command dispatch to the command handler's handle() class method
    or the router's dispatch() method, and maps domain errors to gRPC status codes:
    - CommandRejectedError -> FAILED_PRECONDITION
    - ValueError -> INVALID_ARGUMENT
    """

    def __init__(self, handler: type[CommandHandler] | CommandHandlerRouter) -> None:
        if isinstance(handler, type) and issubclass(handler, CommandHandler):
            self._handle = handler.handle
            self._replay: (
                Callable[
                    [command_handler.ReplayRequest], command_handler.ReplayResponse
                ]
                | None
            ) = handler.replay
            self._domain = handler.domain
        else:
            self._handle = handler.dispatch
            self._replay = None  # CommandHandlerRouter doesn't support replay
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
    ) -> command_handler.BusinessResponse:
        try:
            return self._handle(request)
        except CommandRejectedError as e:
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))
        except ValueError as e:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(e))

    def Replay(
        self,
        request: command_handler.ReplayRequest,
        context: grpc.ServicerContext,
    ) -> command_handler.ReplayResponse:
        """Replay events to compute state (for conflict detection).

        Only available for CommandHandler class handlers, not CommandHandlerRouter.
        """
        if self._replay is None:
            context.abort(
                grpc.StatusCode.UNIMPLEMENTED,
                "Replay not supported for router-based command handlers",
            )
        try:
            return self._replay(request)
        except Exception as e:
            context.abort(grpc.StatusCode.INTERNAL, str(e))


def run_command_handler_server(
    handler: type[CommandHandler] | CommandHandlerRouter | CommandRouter,
    default_port: str,
    logger: structlog.BoundLogger | None = None,
) -> None:
    """Start a gRPC server for a command handler.

    Args:
        handler: Either a CommandHandler subclass or a CommandHandlerRouter.
        default_port: Default TCP port if PORT env not set.
        logger: Optional structlog logger.
    """
    command_handler_grpc = CommandHandlerGrpc(handler)
    run_server(
        command_handler_pb2_grpc.add_CommandHandlerServiceServicer_to_server,
        command_handler_grpc,
        service_name="CommandHandler",
        domain=command_handler_grpc.domain,
        default_port=default_port,
        logger=logger,
    )
