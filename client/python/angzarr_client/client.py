"""Client implementations for Angzarr gRPC services."""

import os
from typing import Optional

import grpc

from .proto.angzarr import (
    AggregateCoordinatorServiceStub,
    EventQueryServiceStub,
    SpeculativeServiceStub,
    CommandBook,
    CommandResponse,
    SyncCommandBook,
    DryRunRequest,
    EventBook,
    Query,
    Projection,
    SagaResponse,
    ProcessManagerHandleResponse,
    SpeculateProjectorRequest,
    SpeculateSagaRequest,
    SpeculatePmRequest,
)
from .errors import GRPCError, TransportError


def _create_channel(endpoint: str) -> grpc.Channel:
    """Create a gRPC channel for the given endpoint.

    Supports both TCP (host:port) and Unix Domain Sockets (file paths).
    UDS paths are detected by leading '/' or './' and converted to unix:// URIs.
    """
    if endpoint.startswith("/") or endpoint.startswith("./"):
        # Unix domain socket path - convert to gRPC URI format
        return grpc.insecure_channel(f"unix://{endpoint}")
    elif endpoint.startswith("unix://"):
        # Already in URI format
        return grpc.insecure_channel(endpoint)
    else:
        # TCP endpoint (host:port)
        return grpc.insecure_channel(endpoint)


class QueryClient:
    """Client for the EventQueryService."""

    def __init__(self, channel: grpc.Channel):
        self._stub = EventQueryServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "QueryClient":
        """Connect to an event query service at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "QueryClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def get_event_book(self, query: Query) -> EventBook:
        """Retrieve a single EventBook for the query."""
        try:
            return self._stub.GetEventBook(query)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def get_events(self, query: Query) -> list[EventBook]:
        """Retrieve all EventBooks matching the query."""
        try:
            return list(self._stub.GetEvents(query))
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class AggregateClient:
    """Client for the AggregateCoordinatorService."""

    def __init__(self, channel: grpc.Channel):
        self._stub = AggregateCoordinatorServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "AggregateClient":
        """Connect to an aggregate coordinator at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "AggregateClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def handle(self, command: CommandBook) -> CommandResponse:
        """Execute a command asynchronously."""
        try:
            return self._stub.Handle(command)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def handle_sync(self, command: SyncCommandBook) -> CommandResponse:
        """Execute a command synchronously with the specified sync mode."""
        try:
            return self._stub.HandleSync(command)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def dry_run_handle(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command in dry-run mode (no persistence)."""
        try:
            return self._stub.DryRunHandle(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class SpeculativeClient:
    """Client for the SpeculativeService."""

    def __init__(self, channel: grpc.Channel):
        self._stub = SpeculativeServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "SpeculativeClient":
        """Connect to a speculative service at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "SpeculativeClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def dry_run(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command without persistence."""
        try:
            return self._stub.DryRunCommand(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def projector(self, request: SpeculateProjectorRequest) -> Projection:
        """Speculatively execute a projector against events."""
        try:
            return self._stub.SpeculateProjector(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def saga(self, request: SpeculateSagaRequest) -> SagaResponse:
        """Speculatively execute a saga against events."""
        try:
            return self._stub.SpeculateSaga(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def process_manager(
        self, request: SpeculatePmRequest
    ) -> ProcessManagerHandleResponse:
        """Speculatively execute a process manager."""
        try:
            return self._stub.SpeculateProcessManager(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class DomainClient:
    """Combined client for aggregate and query operations on a single domain."""

    def __init__(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "DomainClient":
        """Connect to a domain's coordinator at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "DomainClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def execute(self, command: CommandBook) -> CommandResponse:
        """Execute a command (delegates to aggregate client)."""
        return self.aggregate.handle(command)

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class Client:
    """Combined client for aggregate, query, and speculative operations."""

    def __init__(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "Client":
        """Connect to a server providing all services."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "Client":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()
