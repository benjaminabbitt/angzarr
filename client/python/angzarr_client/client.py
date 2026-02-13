"""Client implementations for Angzarr gRPC services."""

import os
from typing import Optional

import grpc

from .proto.angzarr import (
    AggregateCoordinatorServiceStub,
    SagaCoordinatorServiceStub,
    ProjectorCoordinatorServiceStub,
    ProcessManagerCoordinatorServiceStub,
    EventQueryServiceStub,
    CommandBook,
    CommandResponse,
    SyncCommandBook,
    SpeculateAggregateRequest,
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
    UDS paths are detected by leading '/' or './' and converted to unix: URIs.
    Note: grpc-python uses unix:path for relative, unix:///path for absolute.

    KNOWN LIMITATION: grpc-python 1.57+ has a cross-language interoperability
    issue with tonic (Rust) servers over Unix Domain Sockets. Connections fail
    with "RST_STREAM with error code 1" (PROTOCOL_ERROR). This is tracked in:
    - https://github.com/hyperium/tonic/issues/826
    - https://github.com/hyperium/tonic/issues/742
    - https://github.com/grpc/grpc/issues/34760

    Workaround: Use TCP instead of UDS when Python clients connect to Rust
    (tonic) servers. UDS works fine for same-language communication.
    """
    if endpoint.startswith("./"):
        # Relative Unix domain socket path - use unix:path format
        return grpc.insecure_channel(f"unix:{endpoint}")
    elif endpoint.startswith("/"):
        # Absolute Unix domain socket path - use unix:///path format
        return grpc.insecure_channel(f"unix://{endpoint}")
    elif endpoint.startswith("unix:"):
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

    def handle_sync_speculative(
        self, request: SpeculateAggregateRequest
    ) -> CommandResponse:
        """Execute a command speculatively against temporal state (no persistence)."""
        try:
            return self._stub.HandleSyncSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class SpeculativeClient:
    """Client for speculative operations across coordinator services.

    Speculative execution runs commands/events against temporal state without persistence.
    Each coordinator service now provides its own speculative method.
    """

    def __init__(self, channel: grpc.Channel):
        self._aggregate_stub = AggregateCoordinatorServiceStub(channel)
        self._saga_stub = SagaCoordinatorServiceStub(channel)
        self._projector_stub = ProjectorCoordinatorServiceStub(channel)
        self._pm_stub = ProcessManagerCoordinatorServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "SpeculativeClient":
        """Connect to coordinator services at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "SpeculativeClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def aggregate(self, request: SpeculateAggregateRequest) -> CommandResponse:
        """Execute a command speculatively against temporal state."""
        try:
            return self._aggregate_stub.HandleSyncSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def projector(self, request: SpeculateProjectorRequest) -> Projection:
        """Speculatively execute a projector against events."""
        try:
            return self._projector_stub.HandleSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def saga(self, request: SpeculateSagaRequest) -> SagaResponse:
        """Speculatively execute a saga against events."""
        try:
            return self._saga_stub.ExecuteSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def process_manager(
        self, request: SpeculatePmRequest
    ) -> ProcessManagerHandleResponse:
        """Speculatively execute a process manager."""
        try:
            return self._pm_stub.HandleSpeculative(request)
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


